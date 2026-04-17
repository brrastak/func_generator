#![deny(unsafe_code)]
#![no_main]
#![no_std]


use cortex_m::singleton;
use embedded_hal::pwm::SetDutyCycle;
use waveshare_rp2040_zero as rp;
use embedded_hal::digital::OutputPin;
use hal::{
    clocks::{init_clocks_and_plls, Clock},
    fugit::{RateExtU32, TimerInstantU64},
    gpio,
    pio::PIOExt,
    Sio,
    timer::{Timer, Alarm},
    Watchdog,
};
use panic_halt as _;
use rp::hal;
use rp::{Pins, XOSC_CRYSTAL_FREQ};
use rtic_monotonics::systick::prelude::*;
use rtic_sync::{channel::*, make_channel};
use smart_leds::{SmartLedsWrite, RGB8};
use ws2812_pio::Ws2812;


systick_monotonic!(Mono, 1000);



#[rtic::app(device = hal::pac, peripherals = true, dispatchers = [SW0_IRQ])]
mod app {

    use super::*;


    type RgbLed = Ws2812<hal::pac::PIO0, hal::pio::SM0, hal::timer::CountDown, hal::gpio::Pin<hal::gpio::bank0::Gpio16, hal::gpio::FunctionPio0, hal::gpio::PullDown>>;
    type Pwm = hal::pwm::Slice<hal::pwm::Pwm4, hal::pwm::FreeRunning>;
    type PwmAlarm = hal::timer::Alarm0;
    type PwmChannel = &'static mut hal::pwm::Channel<hal::pwm::Slice<hal::pwm::Pwm4, hal::pwm::FreeRunning>, hal::pwm::A>;


    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        alarm: PwmAlarm,
        channel: PwmChannel,
        rgb_led: RgbLed,
        duty_receiver: Receiver<'static, u16, 1>,
        alarm_timer: Timer,
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

        let out = pins.gp8.into_push_pull_output();

        // Configure timer
        // For RGB LED
        let delay = Timer::new(cx.device.TIMER, &mut resets, &clocks);
        // For PWM adjust period
        let mut alarm_timer = delay.clone();

        // Configure the addressable LED
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

        let pwm: &'static mut _ = singleton!(: Pwm = pwm_slices.pwm4).unwrap();
        pwm.set_ph_correct();
        pwm.enable();

        let channel = &mut pwm.channel_a;
        channel.output_to(out);

        // Update the PWM duty cycle every 100 us
        let mut alarm = alarm_timer.alarm_0().unwrap();
        alarm.schedule(100.micros()).ok();
        alarm.enable_interrupt();


        // New duty cycle value
        let (duty_sender, duty_receiver) = make_channel!(u16, 1);


        rgb_led::spawn().ok();
        generate::spawn(duty_sender).ok();

        (
            Shared {},
            Local {
                alarm,
                channel,
                rgb_led,
                duty_receiver,
                alarm_timer,
            },
        )
    }


    // Generate duty cycle values
    #[task(priority = 1)]
    async fn generate(_cx: generate::Context, mut duty_sender: Sender<'static, u16, 1>) {

        loop {

            duty_sender.send(32768).await.ok();
            Mono::delay(1000.millis()).await;

            duty_sender.send(0).await.ok();
            Mono::delay(1000.millis()).await;
        }
    }


    // Update PWM duty cycle every 100 us
    #[task(binds = TIMER_IRQ_0, local = [alarm, channel, alarm_timer, duty_receiver], priority = 1)]
    fn update_pwm(cx: update_pwm::Context) {

        let update_pwm::LocalResources
            {alarm, channel, alarm_timer, duty_receiver, ..} = cx.local;

        let now = alarm_timer.get_counter();
        alarm.clear_interrupt();

        if let Ok(duty_value) = duty_receiver.try_recv() {
            channel.set_duty_cycle(duty_value).unwrap();
        }

        let next = now + 100.micros();
        alarm.schedule_at(next).ok();
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


    #[idle]
    fn idle(_: idle::Context) -> ! {

        loop {
            continue;
        }
    }
}

