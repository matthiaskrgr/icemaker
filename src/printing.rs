use std::io::Write;

use crate::ice::*;

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

pub(crate) fn print_to_stdout(msg: PrintMessage) {
    // todo: perhaps buffer the previous println and if we know  current index, number and file_name == prev don't print at all..? :thinking:
    // because then we don't need to refresh stdout unneccessarily BUT all this would require to be threadsave

    match msg {
        // displays "%perc Checking $file ..."
        PrintMessage::Progress {
            index,
            total_number_of_files,
            file_name,
        } => {
            let perc = ((index * 100) as f32 / total_number_of_files as f32) as u8;

            // do not print a newline so we can (\r-eturn carry) our next status update to the same line, requires flushing though
            // we actually need to print a number of space at the end to "clear" remains of previous lines if previous filename was much longer
            print!("\r[{index}/{total_number_of_files} {perc}%] Checking {file_name: <150}",);
            // kinda ignore whether this fails or not
            let _stdout = std::io::stdout().flush();
        }
        PrintMessage::IceFound { ice } => {
            let _stdout = std::io::stdout().flush();

            // ices are important, make this eprintln?
            println!("\n{}", ice);
        }
    }
}
