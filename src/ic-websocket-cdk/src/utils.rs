#[cfg(not(test))]
use ic_cdk::api::time;

#[macro_export]
macro_rules! custom_print {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        {
            ic_cdk::print(format!("[IC-WEBSOCKET-CDK]: {}", format!($($arg)*)));
        }
        #[cfg(test)]
        {
            println!("[IC-WEBSOCKET-CDK]: {}", format!($($arg)*));
        }
    }
}

#[macro_export]
macro_rules! custom_trap {
    ($($arg:tt)*) => {
        #[cfg(not(test))]
        {
            ic_cdk::trap($($arg)*);
        }
        #[cfg(test)]
        {
            panic!($($arg)*);
        }
    }
}

pub(crate) fn get_current_time() -> u64 {
    #[cfg(test)]
    {
        0u64
    }
    #[cfg(not(test))]
    {
        time()
    }
}
