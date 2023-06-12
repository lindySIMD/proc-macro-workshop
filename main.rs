// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run

use bitfield::*;

#[bitfield]
pub struct MyFourBytes {
    a: B1,
    b: B3,
    c: B4,
    d: B24,
}

fn main() {
    assert_eq!(std::mem::size_of::<MyFourBytes>(), 4);
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
