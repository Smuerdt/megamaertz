#![feature(alloc, collections)]

#![no_std]
#![no_main]

// initialization routing for .data and .bss
extern crate r0;
extern crate stm32f7_discovery as stm32f7;

extern crate collections;
extern crate alloc;

pub mod renderer;
pub mod seven_segment;

use stm32f7::{system_clock, sdram, lcd, i2c, audio, touch, board, embedded};

static TRUMP: &'static [u8] = include_bytes!("../pics/trump.dump");
static TRUMP_SIZE: (u16, u16) = (42, 50);

#[no_mangle]
pub unsafe extern "C" fn reset() -> ! {
    extern "C" {
        static __DATA_LOAD: u32;
        static __DATA_END: u32;
        static mut __DATA_START: u32;
        static mut __BSS_START: u32;
        static mut __BSS_END: u32;
    }

    let data_load = &__DATA_LOAD;
    let data_start = &mut __DATA_START;
    let data_end = &__DATA_END;
    let bss_start = &mut __BSS_START;
    let bss_end = &__BSS_END;

    // initializes the .data section
    //(copy the data segment initializers from flash to RAM)
    r0::init_data(data_start, data_end, data_load);
    // zeroes the .bss section
    r0::zero_bss(bss_start, bss_end);

    stm32f7::heap::init();

    // enable floating point unit
    let scb = stm32f7::cortex_m::peripheral::scb_mut();
    scb.cpacr.modify(|v| v | 0b1111 << 20);

    main(board::hw());
}

fn main(hw: board::Hardware) -> ! {
    let board::Hardware { rcc,
                          pwr,
                          flash,
                          fmc,
                          ltdc,
                          gpio_a,
                          gpio_b,
                          gpio_c,
                          gpio_d,
                          gpio_e,
                          gpio_f,
                          gpio_g,
                          gpio_h,
                          gpio_i,
                          gpio_j,
                          gpio_k,
                          i2c_3,
                          sai_2,
                          .. } = hw;

    use embedded::interfaces::gpio::{self, Gpio};
    let mut gpio = Gpio::new(gpio_a,
                             gpio_b,
                             gpio_c,
                             gpio_d,
                             gpio_e,
                             gpio_f,
                             gpio_g,
                             gpio_h,
                             gpio_i,
                             gpio_j,
                             gpio_k);

    system_clock::init(rcc, pwr, flash);
    // enable all gpio ports
    rcc.ahb1enr.update(|r| {
        r.set_gpioaen(true);
        r.set_gpioben(true);
        r.set_gpiocen(true);
        r.set_gpioden(true);
        r.set_gpioeen(true);
        r.set_gpiofen(true);
        r.set_gpiogen(true);
        r.set_gpiohen(true);
        r.set_gpioien(true);
        r.set_gpiojen(true);
        r.set_gpioken(true);
    });


    // configure led pin as output pin
    let led_pin = (gpio::Port::PortI, gpio::Pin::Pin1);
    let mut led = gpio.to_output(led_pin,
                                 gpio::OutputType::PushPull,
                                 gpio::OutputSpeed::Low,
                                 gpio::Resistor::NoPull)
        .expect("led pin already in use");

    // turn led on
    led.set(true);


    // init sdram (needed for display buffer)
    sdram::init(rcc, fmc, &mut gpio);

    // lcd controller
    let mut lcd = lcd::init(ltdc, rcc, &mut gpio);
    lcd.clear_screen();
    lcd.set_background_color(lcd::Color::rgb(0, 200, 0));

    //i2c
    i2c::init_pins_and_clocks(rcc, &mut gpio);
    let mut i2c_3 = i2c::init(i2c_3);
    touch::check_family_id(&mut i2c_3).unwrap();

    //renderer
    let mut rend = renderer::Renderer::new(&mut lcd);

    // sai and stereo microphone
    audio::init_sai_2_pins(&mut gpio);
    audio::init_sai_2(sai_2, rcc);
    assert!(audio::init_wm8994(&mut i2c_3).is_ok());

    
    // for testing a "rnd" img
    let img = [0xFF; 200];
    let img_clr = [0x00; 200];

    rend.draw_bg_unicolor(0, 0, 150, 272, renderer::RGBColor::from_rgb(255, 0, 0));
    rend.draw_bg_unicolor(165, 0, 150, 272, renderer::RGBColor::from_hex_with_alpha(0xFFFF0000));
    rend.draw_bg_unicolor(330, 0, 150, 272, renderer::RGBColor::from_hex(0xFF0000));

    let mut x = 0;
    let mut y = 0;
    rend.draw(x * 10, y * 10, 10, &img);
    let mut ss_display = seven_segment::SSDisplay::new(100, 100);

    let last_ssd_render_time = system_clock::ticks();
    let mut counter: u16 = 0;
    let mut x = 0;
    loop {
        rend.draw(x * 10 , y * 10, 10, &img_clr);
        x = x + 1;
        if x > 48 {
            x = 0;
            y = (y + 1) % 27;
        }
        rend.draw(x * 10, y * 10, 10, &img);

        if system_clock::ticks() - last_ssd_render_time >= 1000 {
            let ss_pixel = ss_display.render(counter, 0xffff);
            for p in ss_pixel.iter()  {
                rend.render_pixel(p.0, p.1, p.2);
            }
            counter = (counter % core::u16::MAX) + 1;
        }

        // draw image
        rend.draw(200, 100, 10, &img);
        rend.draw_bg(195, 85, 10, &img);
        rend.remove_last_cursor();
       
        // poll for new audio data
        while !sai_2.bsr.read().freq() {} // fifo_request_flag
        let mic_data = sai_2.bdr.read().data();
        let mic_data = mic_data as u16 / 272;

        for i in 0..mic_data {
            rend.render_pixel(i, x, 0xFFFF);
        }
            x += 1;

        // draw cursor where screen is touched
        for touch in &touch::touches(&mut i2c_3).unwrap() {
            rend.cursor(touch.x, touch.y);
        }
    }
}
