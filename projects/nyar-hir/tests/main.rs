use rand::seq::SliceRandom;

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum PokerColor {
    Spade,
    Heart,
    Diamond,
    Club,
    Joker,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Poker {
    number: u8,
    color: PokerColor,
}

impl Poker {
    pub fn cards() -> Vec<Poker> {
        let mut cards = Vec::with_capacity(54);
        for i in 1..=13 {
            cards.push(Poker { number: i, color: PokerColor::Spade });
            cards.push(Poker { number: i, color: PokerColor::Heart });
            cards.push(Poker { number: i, color: PokerColor::Diamond });
            cards.push(Poker { number: i, color: PokerColor::Club });
        }
        cards.push(Poker { number: 14, color: PokerColor::Joker });
        cards.push(Poker { number: 15, color: PokerColor::Joker });
        cards
    }
}

pub fn count_pairs(cards: &[Poker]) -> usize {
    let mut count = 0;
    let mut last = 0;
    for card in cards {
        if card.number == last {
            count += 1;
        }
        last = card.number;
    }
    count
}

pub fn count_pair2(cards: &[Poker]) -> usize {
    let mut count = 0;
    let mut last = 0;
    let mut count_last = 0;
    for card in cards {
        if card.number == last {
            count_last += 1;
        }
        else {
            count += match count_last {
                2 | 3 => 0,
                4 => 1,
                _ => 0,
            };
            count_last = 1;
        }
        last = card.number;
    }
    count
}

#[test]
fn main() {
    let mut rng = rand::thread_rng();
    let mut all = 0;
    const N: usize = 1000000;
    for _ in 0..N {
        let mut cards = Poker::cards();
        cards.shuffle(&mut rng);
        let count = count_pair2(&cards);
        all += count;
    }
    println!("avg count: {}", all as f64 / N as f64);
}
