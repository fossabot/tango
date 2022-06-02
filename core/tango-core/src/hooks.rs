use crate::{facade, fastforwarder, shadow};

mod bn6;

lazy_static! {
    pub static ref HOOKS: std::collections::HashMap<String, &'static Box<dyn Hooks + Send + Sync>> = {
        let mut hooks =
            std::collections::HashMap::<String, &'static Box<dyn Hooks + Send + Sync>>::new();
        hooks.insert("MEGAMAN6_FXX".to_string(), &bn6::MEGAMAN6_FXX);
        hooks.insert("MEGAMAN6_GXX".to_string(), &bn6::MEGAMAN6_GXX);
        hooks.insert("ROCKEXE6_RXX".to_string(), &bn6::ROCKEXE6_RXX);
        hooks.insert("ROCKEXE6_GXX".to_string(), &bn6::ROCKEXE6_GXX);
        hooks
    };
}

pub trait Hooks {
    fn fastforwarder_traps(
        &'static self,
        ff_state: fastforwarder::State,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)>;

    fn shadow_traps(
        &'static self,
        shadow_state: shadow::State,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)>;

    fn primary_traps(
        &'static self,
        handle: tokio::runtime::Handle,
        joyflags: std::sync::Arc<std::sync::atomic::AtomicU32>,
        facade: facade::Facade,
    ) -> Vec<(u32, Box<dyn FnMut(mgba::core::CoreMutRef)>)>;

    fn replace_opponent_name(&'static self, core: mgba::core::CoreMutRef, name: &str);

    fn raw_input_size(&'static self) -> u8;

    fn set_joyflags_in_baked(&'static self, baked: &mut [u8], joyflags: u16);

    fn joyflags_in_baked(&'static self, baked: &[u8]) -> u16;

    fn current_tick(&'static self, core: mgba::core::CoreMutRef) -> u32;
}
