#![deny(unsafe_code)]
#![no_main]
#![no_std]


use embedded_hal::pwm::SetDutyCycle;
use waveshare_rp2040_zero as rp;
use hal::{
    clocks::{init_clocks_and_plls, Clock},
    pio::PIOExt,
    Sio,
    timer::Timer,
    Watchdog,
};
use panic_halt as _;
use rp::hal;
use rp::{Pins, XOSC_CRYSTAL_FREQ};
use rtic_monotonics::systick::prelude::*;
use rtic_sync::{channel::*, make_channel};
use smart_leds::{SmartLedsWrite, RGB8};
use ws2812_pio::Ws2812;

use func_generator::generator::SinGenerator;


systick_monotonic!(Mono, 100_000);



#[rtic::app(device = hal::pac, peripherals = true, dispatchers = [SW0_IRQ])]
mod app {

    use super::*;


    type RgbLed = Ws2812<hal::pac::PIO0, hal::pio::SM0, hal::timer::CountDown, hal::gpio::Pin<hal::gpio::bank0::Gpio16, hal::gpio::FunctionPio0, hal::gpio::PullDown>>;
    type Pwm = hal::pwm::Slice<hal::pwm::Pwm4, hal::pwm::FreeRunning>;


    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        pwm: Pwm,
        duty_receiver: Receiver<'static, u16, 1>,
        rgb_led: RgbLed,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {

        let mut resets = cx.device.RESETS;
        let mut watchdog = Watchdog::new(cx.device.WATCHDOG);
        let clocks = init_clocks_and_plls(
            XOSC_CRYSTAL_FREQ,
            cx.device.XOSC,
            cx.device.CLOCKS,
            cx.device.PLL_SYS,
            cx.device.PLL_USB,
            &mut resets,
            &mut watchdog,
        )
        .ok()
        .unwrap();

        let sio = Sio::new(cx.device.SIO);
        let pins = Pins::new(
            cx.device.IO_BANK0,
            cx.device.PADS_BANK0,
            sio.gpio_bank0,
            &mut resets,
        );

        Mono::start(cx.core.SYST, clocks.system_clock.freq().to_Hz());

        // Configure the addressable LED
        let delay = Timer::new(cx.device.TIMER, &mut resets, &clocks);
        let (mut pio, sm0, _, _, _) = cx.device.PIO0.split(&mut resets);
        let rgb_led = Ws2812::new(
            pins.neopixel.into_function(),
            &mut pio,
            sm0,
            clocks.peripheral_clock.freq(),
            delay.count_down(),
        );

        // Configure the PWM for the generator output
        let pwm_slices = hal::pwm::Slices::new(cx.device.PWM, &mut resets);
        let mut pwm = pwm_slices.pwm4;
        pwm.set_ph_correct();
        let out = pins.gp8.into_push_pull_output();
        pwm.channel_a.output_to(out);
        pwm.set_div_int(1);
        // PWM frequency = 50 kHz
        pwm.set_top(1250);
        pwm.enable();
        pwm.enable_interrupt();

        // New duty cycle value
        let (duty_sender, duty_receiver) = make_channel!(u16, 1);


        rgb_led::spawn().ok();
        generate::spawn(duty_sender).ok();

        (
            Shared {},
            Local {
                pwm,
                duty_receiver,
                rgb_led,
            },
        )
    }


    // Generate duty cycle values
    #[task(priority = 1)]
    async fn generate(_cx: generate::Context, mut duty_sender: Sender<'static, u16, 1>) {

        // 50Hz
        let signal_period: fugit::Duration<u32, 1, 1_000_000> = 20.millis();
        let pwm_period: fugit::Duration<u32, 1, 1_000_000> = 20.micros();
        let max_duty_value = 1200.0;

        let generator = SinGenerator::new(signal_period, pwm_period, max_duty_value).unwrap();
        let values = generator.get_values();

        loop {
            for value in &values {
                duty_sender.send(*value).await.ok();
            }
        }
    }


    // Update PWM duty cycle every 20 us
    #[task(binds = PWM_IRQ_WRAP, local = [pwm, duty_receiver], priority = 1)]
    fn update_pwm(cx: update_pwm::Context) {

        let update_pwm::LocalResources
            {pwm, duty_receiver, ..} = cx.local;

        if let Ok(duty_value) = duty_receiver.try_recv() {
            pwm.channel_a.set_duty_cycle(duty_value).unwrap();
        }

        pwm.clear_interrupt();
    }


    // Heartbit blink
    #[task(local = [rgb_led], priority = 1)]
    async fn rgb_led(cx: rgb_led::Context) {

        let rgb_led::LocalResources
            {rgb_led, ..} = cx.local;

        let color = RGB8::new(1, 0, 0);
        let off_color = RGB8::new(0, 0, 0);

        loop {

            rgb_led.write([color]).ok();
            Mono::delay(1000.millis()).await;

            rgb_led.write([off_color]).ok();
            Mono::delay(1000.millis()).await;
        }
    }


    #[idle()]
    fn idle(_: idle::Context) -> ! {

        loop {

            continue;
        }
    }
}

