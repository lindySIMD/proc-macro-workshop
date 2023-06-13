// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run

// use bitfield::*;

// #[bitfield]
// pub struct MyFourBytes {
//     a: B1,
//     b: B3,
//     c: B4,
//     d: B24,
// }

// fn main() {
//     assert_eq!(std::mem::size_of::<MyFourBytes>(), 4);
// }
// use bitfield::*;
// use std::mem::size_of_val;

// type A = B1;
// type B = B3;
// type C = B4;
// type D = B24;

// #[bitfield]
// pub struct MyFourBytes {
//     a: A,
//     b: B,
//     c: C,
//     d: D,
// }

// fn main() {
//     let mut x = MyFourBytes::new();

//     // I am testing the signatures in this roundabout way to avoid making it
//     // possible to pass this test with a generic signature that is inconvenient
//     // for callers, such as `fn get_a<T: From<u64>>(&self) -> T`.

//     let a = 1;
//     x.set_a(a); // expect fn(&mut MyFourBytes, u8)
//     let b = 1;
//     x.set_b(b);
//     let c = 1;
//     x.set_c(c);
//     let d = 1;
//     x.set_d(d); // expect fn(&mut MyFourBytes, u32)

//     assert_eq!(size_of_val(&a), 1);
//     assert_eq!(size_of_val(&b), 1);
//     assert_eq!(size_of_val(&c), 1);
//     assert_eq!(size_of_val(&d), 4);

//     assert_eq!(size_of_val(&x.get_a()), 1); // expect fn(&MyFourBytes) -> u8
//     assert_eq!(size_of_val(&x.get_b()), 1);
//     assert_eq!(size_of_val(&x.get_c()), 1);
//     assert_eq!(size_of_val(&x.get_d()), 4); // expect fn(&MyFourBytes) -> u32
// }
use bitfield::*;

#[bitfield]
pub struct RedirectionTableEntry {
    acknowledged: bool,
    trigger_mode: TriggerMode,
    delivery_mode: DeliveryMode,
    reserved: B3,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
pub enum TriggerMode {
    Edge = 0,
    Level = 1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq)]
pub enum DeliveryMode {
    Fixed = 0b000,
    Lowest = 0b001,
    SMI = 0b010,
    RemoteRead = 0b011,
    NMI = 0b100,
    Init = 0b101,
    Startup = 0b110,
    External = 0b111,
}

fn main() {
    assert_eq!(std::mem::size_of::<RedirectionTableEntry>(), 1);

    // Initialized to all 0 bits.
    let mut entry = RedirectionTableEntry::new();
    assert_eq!(entry.get_acknowledged(), false);
    assert_eq!(entry.get_trigger_mode(), TriggerMode::Edge);
    assert_eq!(entry.get_delivery_mode(), DeliveryMode::Fixed);

    entry.set_acknowledged(true);
    entry.set_delivery_mode(DeliveryMode::SMI);
    assert_eq!(entry.get_acknowledged(), true);
    assert_eq!(entry.get_trigger_mode(), TriggerMode::Edge);
    assert_eq!(entry.get_delivery_mode(), DeliveryMode::SMI);
}

// use sorted::sorted;

// #[sorted]
// pub enum Conference {
//     RustBeltRust,
//     RustConf,
//     RustFest,
//     RustLatam,
//     RustRush,
// }

// impl Conference {
//     #[sorted::check]
//     pub fn region(&self) -> &str {
//         use self::Conference::*;

//         #[sorted]
//         match self {
//             RustFest => "Europe",
//             RustLatam => "Latin America",
//             _ => "elsewhere",
//         }
//     }
// }

// fn main() {}

// use sorted::sorted;

// #[sorted]
// pub enum Conference {
//     RustBeltRust,
//     RustConf,
//     RustFest,
//     RustLatam,
//     RustRush,
// }

// fn main() {}

// use derive_builder::Builder;

// #[derive(Builder)]
// pub struct Command {
//     executable: String,
//     #[builder(each = "arg")]
//     args: Vec<String>,
//     env: Vec<String>,
//     current_dir: Option<String>,
// }

// fn main() {
//     let builder = Command::builder();

//     let _ = builder;
// }

// // fn main() {}
