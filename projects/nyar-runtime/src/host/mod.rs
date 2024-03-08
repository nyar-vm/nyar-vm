use crate::unstable::debugger::print::{Errno, Human, Permissions, Point};

pub struct NyarExtension {}

impl crate::unstable::debugger::print::Host for NyarExtension {
    fn print_i8(&mut self, i: i8) -> anyhow::Result<()> {
        println!("{}", i);
        Ok(())
    }

    fn print_i16(&mut self, i: i16) -> anyhow::Result<()> {
        println!("{}", i);
        Ok(())
    }

    fn print_i32(&mut self, i: i32) -> anyhow::Result<()> {
        println!("{}", i);
        Ok(())
    }

    fn print_i64(&mut self, i: i64) -> anyhow::Result<()> {
        println!("{}", i);
        Ok(())
    }

    fn print_u8(&mut self, u: u8) -> anyhow::Result<()> {
        println!("{}", u);
        Ok(())
    }

    fn print_u16(&mut self, u: u16) -> anyhow::Result<()> {
        println!("{}", u);
        Ok(())
    }

    fn print_u32(&mut self, u: u32) -> anyhow::Result<()> {
        println!("{}", u);
        Ok(())
    }

    fn print_u64(&mut self, u: u64) -> anyhow::Result<()> {
        println!("{}", u);
        Ok(())
    }

    fn print_f32(&mut self, f: f32) -> anyhow::Result<()> {
        println!("{}", f);
        Ok(())
    }

    fn print_f64(&mut self, f: f64) -> anyhow::Result<()> {
        println!("{}", f);
        Ok(())
    }

    fn print_char(&mut self, c: char) -> anyhow::Result<()> {
        println!("{}", c);
        Ok(())
    }

    fn print_str(&mut self, s: String) -> anyhow::Result<()> {
        println!("{}", s);
        Ok(())
    }

    fn print_tuple(&mut self, value: (u8, i8)) -> anyhow::Result<()> {
        println!("{:?}", value);
        Ok(())
    }

    fn print_point(&mut self, value: Point) -> anyhow::Result<()> {
        println!("{:?}", value);
        Ok(())
    }

    fn print_human(&mut self, value: Human) -> anyhow::Result<()> {
        println!("{:?}", value);
        Ok(())
    }

    fn print_errno(&mut self, value: Errno) -> anyhow::Result<()> {
        println!("{:?}", value);
        Ok(())
    }

    fn print_permissions(&mut self, value: Permissions) -> anyhow::Result<()> {
        println!("{:?}", value);
        Ok(())
    }
}
