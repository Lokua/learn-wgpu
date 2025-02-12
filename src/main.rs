use log::error;

use learn_wgpu::run;

fn main() {
    if let Err(e) = pollster::block_on(run()) {
        error!("Error: {:?}", e);
    }
}
