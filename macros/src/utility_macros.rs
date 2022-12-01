#[macro_export]
macro_rules! acquire_lock {
    ($mutex:expr, $lock:ident => $exec:block ) => {
        match $mutex.lock() {
            #[allow(unused_mut)]
            Ok(mut $lock) => $exec,
            _ => Default::default(),
        }
    };
}
