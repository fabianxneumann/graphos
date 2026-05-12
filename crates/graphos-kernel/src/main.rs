#![no_std]
#![no_main]

use uefi::prelude::*;

#[entry]
fn main() -> Status {
    loop {
        uefi::boot::stall(1_000_000);
    }
}
