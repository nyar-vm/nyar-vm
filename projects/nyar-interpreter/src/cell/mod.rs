//

//

//

//

//
// pub struct Cell {
//
// }
//
// fn pure(a) -> Control {
//     Control::apply(|k|apply(k,a))
// }
//
//
//
//
// fn delayed(a) -> Control {
//     Control::apply(|k|apply(k,a()))
// }
//
// pub struct Step<A> {
//     c: Control<A>,
//     k: Stack<A, A>,
// }
//
// pub struct Cell<F> {
//     inner: F
// }
//
// pub struct Stack<A, B> {
//     frames: Vec<Frame<A, B>>,
//     fields: Vec<Cell<A>>,
//     prompt: u32,
//     tail: Option<Box<Stack<A, B>>>,
// }
//
// impl<A, B> Stack<A, B> {
//     fn is_empty(&self) -> bool {
//         unimplemented!()
//     }
// }
//
// fn apply<A, B>(stack: Stack<A, B>, a: A) -> Step<B> {
//     let mut s = stack;
//     loop {
//         if s.is_empty() {
//             return a;
//         }
//         let fs = s.frames;
//         if fs.is_empty() {
//             s = *s.tail;
//             continue;
//         }
//         let result = fs.head(a);
//         s.frames = fs.tail();
//         return Step {
//             c: result,
//             k: s,
//         };
//     }
// }
//
// /// Delimited Control
// pub struct Control<A> {
//     apply: A,
// }
//
// type Frame<A, B> = fn(A) -> Control<B>;
//
//
// impl<A> Control<A> {
//     pub fn new(f: A) -> Self {
//         Self {
//             apply: f
//         }
//     }
//     // pub fn run(&self) -> Step<A> {
//     //     let step = Step {
//     //         c: self.to_owned(),
//     //         k: Stack {
//     //             frames: vec![],
//     //             fields: vec![],
//     //             prompt: 0,
//     //             tail: None
//     //         }
//     //     };
//     //     trampoline(step)
//     // }
//     // pub fn then<F>(&self, f: F) -> Control<A> {
//     //     Control {
//     //         apply: |k| Step {
//     //             c: self.to_owned(),
//     //             k: flatMap(k, f)
//     //         }
//     //     }
//     // }
//     // pub fn state<F>(&self, f: F) -> Control<A> {
//     //     self.then(|init| withState(init, f))
//     // }
// }
//
//
// // fn with_state(init, f: fn() -> Control<> ) {
// //     let cell = Cell {
// //         inner: init
// //     };
// //     Control {
// //         apply: |k:Stack<>| {
// //             k.fields.push(cell);
// //             Step {
// //                 c: f(cell),
// //                 s: k
// //             }
// //         }
// //     }
// // }
//
// // fn trampoline(res: Step<B>) -> Step<B> {
// //     while res.is_step() {
// //         res = res.c.apply(res.k)
// //     }
// //     return res;
// // }
