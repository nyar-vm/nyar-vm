use mini_csharp::visitor::{Dog, Cat, Fish, sound};
use mini_csharp::tagless::{expr, SetContext, SoundContext, Neg};

#[test]
fn test_visitor() {
    let dog = Dog { name: "nana".to_string() };
    let cat = Cat { name: "nvnv".to_string() };
    let fish = Fish { name: "fish".to_string() };
    sound(&dog);
    sound(&cat);
    sound(&fish);
}

#[test]
fn test_tagless() {
    assert_eq!(expr::<SetContext>(), 45);
    assert_eq!(expr::<SoundContext>(), "(42) + ((1) + (2))");
    assert_eq!(expr::<Neg<SetContext>>(), -45);
    assert_eq!(expr::<Neg<SoundContext>>(), "(-42) + ((-1) + (-2))");
}
