extern crate four;
use std::io;
use four::List;

struct DropdDeConstructor{

}

impl Drop for DropdDeConstructor {
    fn drop(&mut self) {
        println!("Dropping stuff.")
    }
}

fn main() {
    let mut list = List::new();
    (0..10).for_each(|i| list.push_back(i));
    for i in 0..10 {
        println!("{}", list.get(i).unwrap());
    }

    let mut r = 255u8;
    r = r.saturating_add(1u8);
    println!("{r}");

    let a = [1,2,3,4,5,6];
    println!("Please enter an array index.");

    let mut index = String::new();

    io::stdin().read_line(&mut index).expect("Field to read line");
    let index: usize = index.trim().parse().expect("Index entered was not a number.");

    let element = a[index];
    println!("The value of element at {index} is {element}");

    let _ = DropdDeConstructor{};

}

