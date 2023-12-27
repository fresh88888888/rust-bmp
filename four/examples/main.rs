extern crate four;
use four::List;

fn main(){
    let mut list = List::new();
    (0..10).for_each(|i| list.push_back(i));
    for i in 0..10 {
        println!("{}", list.get(i).unwrap());
    }
}