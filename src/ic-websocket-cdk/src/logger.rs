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
