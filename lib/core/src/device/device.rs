use serde_json;

use crate::interface::listener::Listener;

use crate::interface::fsm::Fsm;
use crate::platform::TaskPoolLoader;

use futures::FutureExt;
use crate::device::traits::DeviceActions;
use crate::link::AmManager as AmLinkManager;

use crate::interface::Builder as InterfaceBuilder;

use crate::{subscription, FunctionResult, __platform_error_result};

use crate::interface::Interface;

use super::logger::{self, Logger};

/// A device manage a set of interfaces
/// 
pub struct Device {

    /// Device name
    dev_name: String,
    bench_name: String,


    pub settings: serde_json::Value,

    started: bool,

    
    actions: Box<dyn DeviceActions>,

    // interfaces: Vec<AmRunner>,

    /// Connection link manager
    /// To generate connection links for the interfaces
    connection_link_manager: AmLinkManager,

    platform_services: crate::platform::services::AmServices,

    logger: Logger,
}

impl Device {

    /// Create a new instance of the Device
    /// 
    pub fn new<
        A: Into<String>,
        B: Into<String>,
    >
    (
        dev_name: A,
        bench_name: B,
        settings: serde_json::Value,
        actions: Box<dyn DeviceActions>,

        connection_link_manager: AmLinkManager,
        platform_services: crate::platform::services::AmServices
    ) -> Device 
    {
        let dev_name = dev_name.into();
        let bench_name = bench_name.into();

        // Create the object
        let obj = Device {
            dev_name: dev_name.clone(),
            bench_name: bench_name.clone(),

            settings: settings,

            started: false,

            actions: actions,
            // interfaces: Vec::new(),

            connection_link_manager: connection_link_manager,
            platform_services: platform_services,

            logger: Logger::new(bench_name.clone(), dev_name.clone())
        };

        // Info log
        obj.log_info("Device created");

        // Return the object
        return obj;
    }

    /// Get the device name
    /// 
    #[inline]
    pub fn dev_name(&self) -> &String {
        return &self.dev_name;
    }

    /// Get the bench name
    /// 
    #[inline]
    pub fn bench_name(&self) -> &String {
        return &self.bench_name;
    }

    /// Attach default connection
    ///
    // async fn attach_default_connection(&mut self, interface: AmRunner) {

    //     let c = self.default_connection.as_ref().unwrap();
    //     let mut interface_lock = interface.lock().await;
    //     // let requests = interface_lock.subscription_requests().await;

    //     let topic = interface_lock.get_topic().await;
    //     let att_names = interface_lock.attributes_names().await;

    //     let mut requests = vec![
    //         subscription::Request::new( subscription::ID_PZA, "pza" ),
    //         subscription::Request::new( subscription::ID_PZA_CMDS_SET, &format!("pza/{}/cmds/set", topic) )
    //     ];

    //     for att_name in att_names {
    //         let request = subscription::Request::new( att_name.0, &format!("pza/{}/{}", topic, att_name.1) );
    //         requests.push(request);
    //     }

    //     let x: link::InterfaceHandle = c.lock().await.request_link(requests).await.unwrap();
    //     interface_lock.set_default_link(x).await;

    // }


    /// Create and Start the interfaces
    /// 
    pub async fn start_interfaces(&mut self, task_loader: &mut TaskPoolLoader)
        -> FunctionResult
    {
        // Do nothing if already started
        if self.started {
            self.logger.log_warn("Device already started");
            // return __platform_error_result!("Device already started");
        }
        self.logger.log_info("Start Interfaces...");

        // Get the interface builders
        let r = self.actions.interface_builders(&self);
        // if let Err(e) = builders {
        //     self.log_warn("Error");
        // }
        let builders = r?;

        // Do nothing if no interface in the device
        if builders.len() == 0 {
            self.logger.log_warn("No interface to build, skip device start");
            return Ok(());
        }

        // create interfaces
        for builder in builders {

            self.start_interface(builder, task_loader).await;
            // self.interfaces.push(
            //     interface::Runner::build(builder,
            //         self.dev_name().clone(),
            //         self.bench_name().clone(),
            //         self.connection_link_manager.clone(),
            //         self.platform_services.clone()
            //     ).await
            // );
        }

        // // Start the interfaces
        // let mut interfaces = self.interfaces.clone();
        // for interface in interfaces.iter_mut() {
        //     let itf = interface.clone();
        //     itf.lock().await.start(task_loader).await;
        // }

        // log
        self.logger.log_info("Interfaces started !");
        self.started = true;

        Ok(())
    }



    /// Build & Start an interface
    /// 
    pub async fn start_interface(&self, interface_builder: InterfaceBuilder, task_loader: &mut TaskPoolLoader) {

    
        // Build Interface Base Topic name
        let topic = format!("pza/{}/{}/{}", self.bench_name, self.dev_name, interface_builder.name);
    
        // Get attributes names
        let att_names = interface_builder.subscriber.attributes_names().await;
    
        // Build subscriptions requests
        let mut requests = vec![
            subscription::Request::new( subscription::ID_PZA, "pza" ),
            subscription::Request::new( subscription::ID_PZA_CMDS_SET, &format!("{}/cmds/set", topic) )
        ];
        for att_name in att_names {
            let request = subscription::Request::new( att_name.0, &format!("{}/cmds/{}", topic, att_name.1) );
            requests.push(request);
        }
    
        // Create the link with the connection
        let link = self.connection_link_manager.lock().await.request_link(requests).await.unwrap();



        let interface = 
            Interface::new(interface_builder.name, self.dev_name.clone(), self.bench_name.clone(), interface_builder.itype, interface_builder.version, link.client(), self.platform_services.clone())
                .as_thread_safe();


        // // TODO ! mutex on FSM and listener is useless... only interface must have a lock
        // let fsm = self.fsm.clone();
        // // let listener = self.listener.clone();



        // FSM Task
        let fsm = Fsm::new(interface.clone(), interface_builder.states);
        task_loader.load(fsm.run_task().boxed()).unwrap();



        let listener = Listener::new(interface.clone(), interface_builder.subscriber, link);
        // // Listener Task
        // // Ensure communication with the MQTT connection
        // let interface_name = self.interface.lock().await.name().clone() ;

        // task_loader.load(Listener::task(listener).boxed()).unwrap();
        task_loader.load(listener.run_task().boxed()).unwrap();

        // // Log success
        // self.interface.lock().await.log_info("Interface started");



    }

    /// Log warning
    /// 
    #[deprecated(since = "0.1.0", note = "use clone_logger() instead")]
    #[inline]
    pub fn log_warn<A: Into<String>>(&self, text: A) {
        // println!("DEPRECATED: Device::log_warn() is deprecated, use Device::clone_logger() instead !");
        tracing::warn!(class="Device", bname=self.bench_name, dname=self.dev_name, "{}", text.into());
    }

    /// Log info
    /// 
    #[deprecated(since = "0.1.0", note = "use clone_logger() instead")]
    #[inline]
    pub fn log_info<A: Into<String>>(&self, text: A) {
        // println!("DEPRECATED: Device::log_info() is deprecated, use Device::clone_logger() instead !");
        tracing::info!(class="Device", bname=self.bench_name, dname=self.dev_name, "{}", text.into());
    }

    /// This function allow an external module to use the device logger
    /// The logger is threadsafe and can also safely be cloned
    /// 
    pub fn clone_logger(&self) -> Logger {
        return self.logger.clone();
    }

}


