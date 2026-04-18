
use core::f32::consts::PI;
// ExtU32 is needed to avoid having to write 20_u32.millis() instead of 20.millis
use fugit::{Duration, ExtU32};
use heapless::vec::Vec;
use micromath::F32Ext;


// Maximum number of value to be generated in one period of the signal. It is used to pre-allocate the sine table.
const MAX_STEPS: usize = 1000;

pub struct SinGenerator {
    steps_number: usize,
    max_duty_value: f32,
}

#[derive(Debug)]
pub enum Error {
    TooManySteps,
}

impl SinGenerator {

    pub fn new(
            signal_period: Duration<u32, 1, 1_000_000>, 
            pwm_period: Duration<u32, 1, 1_000_000>, 
            max_duty_value: f32) -> Result<Self, Error> {
            
        let steps_number = (signal_period / pwm_period) as usize;
        if steps_number > MAX_STEPS {
            return Err(Error::TooManySteps);
        }

        Ok(Self {
            steps_number,
            max_duty_value,
        })
    }

    pub fn get_values(&self) -> Vec<u16, MAX_STEPS> {

        let mut sin_table = Vec::new();

        // Sine wave first half of the period
        for index in 0..self.steps_number/2+1 {
            sin_table.push((F32Ext::sin(index as f32 * 2.0 * PI / self.steps_number as f32) * self.max_duty_value) as u16).ok();
        }
        // 0 second half of the period
        for _ in self.steps_number/2+1..self.steps_number {
            sin_table.push(0).ok();
        }

        sin_table
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::{assert_eq, assert_ne};
    

    #[test]
    fn steps_number() {

        let signal_period: fugit::Duration<u32, 1, 1_000_000> = 20.millis();
        let pwm_period: fugit::Duration<u32, 1, 1_000_000> = 20.micros();
        let max_duty_value = 1200.0;

        let generator = SinGenerator::new(signal_period, pwm_period, max_duty_value).unwrap();

        assert_eq!(generator.steps_number, 1000);
    }

    #[test]
    fn values() {
        let signal_period: fugit::Duration<u32, 1, 1_000_000> = 11.millis();
        let pwm_period: fugit::Duration<u32, 1, 1_000_000> = 1.millis();
        let max_duty_value = 1000.0;

        let generator = SinGenerator::new(signal_period, pwm_period, max_duty_value).unwrap();

        let expected_values: Vec<u16, 11> = Vec::from([0, 540, 910, 989, 756, 280, 0, 0, 0, 0, 0]);
        assert_eq!(generator.get_values(), expected_values);
    }
}
