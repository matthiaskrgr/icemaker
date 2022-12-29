use std::io::Write;
use std::sync::RwLock;

use crate::ice::*;

#[derive(Eq, PartialEq, Debug, Clone)]
pub(crate) enum PrintMessage {
    Progress {
        index: usize,
        total_number_of_files: usize,
        file_name: String,
    },
    IceFound {
        ice: ICEDisplay,
    },
}

pub(crate) struct Printer {
    prev: RwLock<PrintMessage>,
    logged_messages: RwLock<Vec<ICEDisplay>>,
}

impl Printer {
    pub(crate) fn log(&self, new: PrintMessage) {
        let prev = self.prev.read().unwrap().clone();

        if new == prev {
            // no new message, nothing to update
            return;
        }

        if let PrintMessage::IceFound { ref ice } = new {
            if self.logged_messages.read().unwrap().contains(ice) {
                // do not log duplicate ICEs
                return;
            }
        }

        match (prev, &new) {
            // displays "%perc Checking $file ..."
            (
                PrintMessage::Progress { .. },
                PrintMessage::Progress {
                    index,
                    total_number_of_files,
                    file_name,
                },
            ) => {
                let perc = ((index * 100) as f32 / *total_number_of_files as f32) as u8;

                // do not print a newline so we can (\r-eturn carry) our next status update to the same line, requires flushing though
                // we actually need to print a number of space at the end to "clear" remains of previous lines if previous filename was much longer
                print!("\r[{index}/{total_number_of_files} {perc}%] Checking {file_name: <150}",);
                let _stdout = std::io::stdout().flush();
                // kinda ignore whether this fails or not
            }
            (PrintMessage::IceFound { .. }, PrintMessage::IceFound { ref ice }) => {
                println!("{ice}");
            }
            (PrintMessage::Progress { .. }, PrintMessage::IceFound { ref ice }) => {
                println!("{ice}");
            }
            (
                PrintMessage::IceFound { .. },
                PrintMessage::Progress {
                    index,
                    total_number_of_files,
                    file_name,
                },
            ) => {
                // let _stdout = std::io::stdout().flush();

                let perc = ((index * 100) as f32 / *total_number_of_files as f32) as u8;

                // do not print a newline so we can (\r-eturn carry) our next status update to the same line, requires flushing though
                // we actually need to print a number of space at the end to "clear" remains of previous lines if previous filename was much longer
                print!("[{index}/{total_number_of_files} {perc}%] Checking {file_name: <150}",);
                let _stdout = std::io::stdout().flush();
            }
        }

        if let PrintMessage::IceFound { ref ice } = new {
            for wait_dur in 0..=10 {
                if let Ok(mut w) = self.logged_messages.try_write() {
                    w.push(ice.clone());
                    break;
                } else {
                    let wait = wait_dur * 10;
                    eprintln!("failed to acquire rwlock, waiting {wait}ms until retry");
                    std::thread::sleep(std::time::Duration::from_millis(wait));
                }
            }
        }

        // if we can't acquire the lock right away, wait 10 ms and retry. Try up to 10 times
        for wait_dur in 0..=10 {
            match self.prev.try_write() {
                Ok(mut w) => {
                    *w = new;
                    break;
                }
                // failed to acquire lock, wait 10 ms and retry
                _ => {
                    let wait = wait_dur * 10;
                    eprintln!("failed to acquire rwlock, waiting {wait}ms until retry");
                    std::thread::sleep(std::time::Duration::from_millis(wait));
                }
            }
        }
    }

    pub(crate) const fn new() -> Self {
        Printer {
            prev: RwLock::new(PrintMessage::Progress {
                index: 0,
                total_number_of_files: 0,
                file_name: String::new(),
            }),
            logged_messages: RwLock::new(Vec::new()),
        }
    }
}
