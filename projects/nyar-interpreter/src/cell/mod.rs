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

// mod cell.stateful;

// trait CPS<A, B> {
//     fn apply(&self,k: impl Continuation<A, B>);
// }
//
// trait Continuation<A, B>: Prog<B> {
//     fn resume(&self, value: A) -> B;
//     fn apply(&self) -> B {
//         self.resume()
//     }
// }
//
// trait Effectful<A, B> {
//     fn apply(&self,a: A) -> B;
// }
//
// trait Prompt<A> {}
//
// trait Prog<A> {
//     fn apply(&self) -> A;
// }

// trait Handler<R, Res>: Prompt<Res> {
//     fn pure(&self, r: R) -> Res;
//     fn handle(&self, prog: impl Prog<R>) {
//         push_prompt(self, || self.pure(prog.apply()))
//     }
//     fn using<A>(&self, body: impl CPS<A, Res>) {
//         self.with_sub_continuation(
//             self,
//             |k| body.apply(
//                 |a| push_prompt(self, || k.resume(a))
//             ),
//         )
//     }
//     fn using_once<A>() {
//         unimplemented!()
//     }
//     fn discard<A>(&self, body: impl CPS<A, Res>) {
//         self.using(body)
//     }
// }

// type Result = std::result<Effect>
//
// #[test]
// fn test() {
//     println!("it works")
// }
//
// #[derive(Debug, Clone)]
// struct Frame {}
//
// impl Frame {
//     fn enter() {
//
//     }
// }
//
// pub struct Value {
//
// }
//
// pub struct EffektError {
//
// }
//
// struct Runtime {
//     stack: Vec<Frame>,
//     result: Value,
//     last_exception: Option<EffektError>,
//     last_frame: Option<Frame>,
// }
//
// impl Default for Runtime {
//     fn default() -> Self {
//         Self {
//             stack: vec![],
//             result: Value {},
//             last_exception: EffektError {},
//             last_frame: None
//         }
//     }
// }
//
// impl Runtime {
//     fn pop(&mut self) -> Option<Frame> {
//         match &self.last_frame {
//             Some(s) => {
//                 self.last_frame = None;
//                 Some(s.to_owned())
//             }
//             None => {
//                 self.stack.pop()
//             }
//         }
//     }
//
//     fn push(&mut self, frame: Frame) {
//         self.push_last_frame();
//         self.last_frame = Some(frame)
//     }
//     fn push_last_frame(&mut self) {
//         match &self.last_frame {
//             Some(s) => {
//                 self.stack.push(s.to_owned())
//             }
//             None => {}
//         }
//     }
//     fn push_prompt<A>(&mut self, prompt: impl Prompt<A>, prog: impl Prog<A>) -> A {
//         self.push_last_frame();
//         self.stack.push_prompt(prompt);
//         prog.apply()
//     }
//     fn with_sub_continuation(&mut self, prompt: impl Prompt<R>, body: impl CPS<A, R>) {
//         self.push_last_frame();
//         let (init, rest) = self.stack.split_at(prompt);
//         self.last_frame = None;
//         self.stack = rest.to_vec();
//         body.apply(SubContinuation::new(init))
//     }
// }
//
// impl Runtime {
//     fn run<A>(&mut self, prog: impl Prog<A>) {
//         self.stack = vec![];
//         self.last_frame = prog.apply();
//         self.trampoline()
//     }
//     fn trampoline(&mut self) {
//         while self.last_frame.is_some() || !self.stack.is_empty() {
//             let result = match &self.last_frame {
//                 Some(f) => {
//                     self.last_frame = None;
//                     f.enter()
//                 }
//                 None => {
//                     let f = self.stack.pop().unwrap();
//                     f.enter()
//                 }
//             };
//             match result {}
//         }
//     }
// }
//
// struct SubContinuation<A,R> {
//     state: Vec<Frame>
// }
//
// impl SubContinuation<A,R> {
//     fn new(init: &[Frame])->Self {
//         Self {
//             state: Vec::from(init)
//         }
//     }
// }
//
// impl Continuation<A,R> for SubContinuation<A,R>{
//     fn resume(&self,  value: A) -> R {
//         let mut r = Runtime::default();
//         r.push_last_frame();
//         r.stack = self.state.prepend_to(r.stack);
//         r.return_with(value)
//     }
// }

// trait Continuation<A, B>: Prog<B> {
//     fn resume(&self, value: A) -> B;
//     fn apply(&self) -> B {
//         self.resume()
//     }
// }

mod interpreter;
