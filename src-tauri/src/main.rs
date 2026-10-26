#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    if let Some(code) = wuhu_lib::run_ticket_extractor_if_requested() {
        std::process::exit(code);
    }
    wuhu_lib::run()
}
