use core;
use constants;
use random::{self, Rng};
use renderer;
use collections::vec::Vec;
use seven_segment::SSDisplay;
use stm32f7::board::sai::Sai;
use stm32f7::system_clock;

pub struct Game<'a> {
    pub evil_targets: Vec<Target>,
    pub hero_targets: Vec<Target>,
    pub rend: &'a mut renderer::Renderer<'a>,
    pub score: u16,
    pub countdown: u16,
    pub rand: random::MTRng32,
    pub tick: usize,
    pub last_super_trump_render_time: usize,
    pub last_ssd_render_time: usize,
    pub ss_ctr_display: SSDisplay,
    pub ss_hs_display: SSDisplay,
}

impl<'a> Game<'a> {
    pub fn init(&mut self) {
        self.ss_hs_display.render(0, 0x8000, self.rend);
    }

    pub fn update_tick(&mut self, tick: usize) {
        self.tick = tick;
    }

    pub fn update_countdown(&mut self) {
        self.update_tick(system_clock::ticks());
        if self.tick - self.last_ssd_render_time >= 1000 {
            self.countdown -= if self.countdown > 0 { 1 } else { 0 };
            self.ss_ctr_display
                .render(self.countdown, 0x8000, self.rend);
            self.last_ssd_render_time = self.tick;
        }
    }

    pub fn draw_missing_targets(&mut self) {
        // rendering random positioned evil evil_targets (trumps)
        while self.evil_targets.len() < 5 {
            let lifetime = Self::get_rnd_lifetime(&mut self.rand);
            let pos: (u16, u16) =
                Self::get_rnd_pos(&mut self.rand, &self.hero_targets, &self.evil_targets);
            let evil_target = Target::new(pos.0,
                                          pos.1,
                                          constants::TARGET_SIZE_50.0,
                                          constants::TARGET_SIZE_50.1,
                                          50,
                                          self.tick,
                                          lifetime);
            let super_evil_target = Target::new(pos.0,
                                                pos.1,
                                                constants::TARGET_SIZE_50.0,
                                                constants::TARGET_SIZE_50.1,
                                                100,
                                                self.tick,
                                                2000);
            if self.tick - self.last_super_trump_render_time >=
               8000 + (self.rand.rand() as usize % 3000) {
                self.rend
                    .draw_dump(pos.0, pos.1, constants::TARGET_SIZE_50, ::SUPER_TRUMP);
                self.last_super_trump_render_time = self.tick;
                self.evil_targets.push(super_evil_target);
            } else {
                self.rend
                    .draw_dump(pos.0, pos.1, constants::TARGET_SIZE_50, ::TRUMP);
                self.evil_targets.push(evil_target);
            }
        }

        // rendering random positioned hero evil_targets (mexicans)
        while self.hero_targets.len() < 3 {
            let lifetime = Self::get_rnd_lifetime(&mut self.rand);
            let pos: (u16, u16) =
                Self::get_rnd_pos(&mut self.rand, &self.hero_targets, &self.evil_targets);
            let hero_target = Target::new(pos.0,
                                          pos.1,
                                          constants::TARGET_SIZE_50.0,
                                          constants::TARGET_SIZE_50.1,
                                          30,
                                          self.tick,
                                          lifetime);
            self.rend
                .draw_dump(pos.0, pos.1, constants::TARGET_SIZE_50, ::MEXICAN);
            self.hero_targets.push(hero_target);
        }
    }

    pub fn process_shooting(&mut self, sai_2: &'static Sai, touches: Vec<(u16, u16)>) {
        if Self::vol_limit_reached(sai_2) {
            let mut hit_evil_targets = Target::check_for_hit(&mut self.evil_targets, &touches);
            hit_evil_targets.sort();
            for hit_index in hit_evil_targets.iter().rev() {
                let t = self.evil_targets.remove(*hit_index);
                self.rend.clear(t.x, t.y, (t.width, t.height));
                self.score += t.bounty;
                self.ss_hs_display
                    .render(self.score, constants::GREEN, self.rend);
            }
            let mut hit_hero_targets = Target::check_for_hit(&mut self.hero_targets, &touches);
            hit_hero_targets.sort();
            for hit_index in hit_hero_targets.iter().rev() {
                let t = self.hero_targets.remove(*hit_index);
                self.rend.clear(t.x, t.y, (t.width, t.height));
                self.score -= if self.score < 30 {
                    self.score
                } else {
                    t.bounty
                };
                self.ss_hs_display
                    .render(self.score, constants::RED, self.rend);
            }
        }
    }

    pub fn purge_old_targets(&mut self) {
        // dont let targets live longer than lifetime secs
        for i in (0..self.evil_targets.len()).rev() {
            if self.tick - self.evil_targets[i].birthday > self.evil_targets[i].lifetime {
                let t = self.evil_targets.remove(i);
                self.rend.clear(t.x, t.y, (t.width, t.height));
            }
        }

        for i in (0..self.hero_targets.len()).rev() {
            if self.tick - self.hero_targets[i].birthday > self.hero_targets[i].lifetime {
                let t = self.hero_targets.remove(i);
                self.rend.clear(t.x, t.y, (t.width, t.height));
            }
        }
    }

    fn vol_limit_reached(sai_2: &'static Sai) -> bool {
        while !sai_2.bsr.read().freq() {} // fifo_request_flag
        let data0 = sai_2.bdr.read().data() as i16 as i32;
        while !sai_2.bsr.read().freq() {} // fifo_request_flag
        let data1 = sai_2.bdr.read().data() as i16 as i32;

        let mic_data = if data0.abs() > data1.abs() {
            data0.abs() as u16
        } else {
            data1.abs() as u16
        };

        // mic_data reprents our "volume". Magic number 420 after testing.
        let blaze_it = 2000;
        mic_data > blaze_it
    }

    fn get_rnd_lifetime(rnd: &mut random::Rng) -> usize {
        let mut num = rnd.rand() as usize;
        num &= 0x3FFF;
        core::cmp::max(num, 5000)
    }

    fn get_rnd_pos(rand: &mut random::Rng,
                   existing_hero: &[Target],
                   existing_evil: &[Target])
                   -> (u16, u16) {
        let mut pos = renderer::Renderer::get_random_pos(rand,
                                                         constants::TARGET_SIZE_50.0,
                                                         constants::TARGET_SIZE_50.1);
        while !Self::pos_is_okay(pos, existing_hero, existing_evil) {
            pos = renderer::Renderer::get_random_pos(rand,
                                                     constants::TARGET_SIZE_50.0,
                                                     constants::TARGET_SIZE_50.1);
        }
        pos
    }

    fn are_overlapping_targets(target: &Target, pos: (u16, u16)) -> bool {
        let corner_ul = (target.x, target.y);
        let corner_lr = (target.x + target.width, target.y + target.height);

        let x1 = pos.0;
        let y1 = pos.1;
        let x2 = pos.0 + constants::TARGET_SIZE_50.0;
        let y2 = pos.1 + constants::TARGET_SIZE_50.1;

        Self::point_is_within((x1, y1), corner_ul, corner_lr) ||
        Self::point_is_within((x2, y2), corner_ul, corner_lr) ||
        Self::point_is_within((x1, y2), corner_ul, corner_lr) ||
        Self::point_is_within((x2, y1), corner_ul, corner_lr)
    }

    fn point_is_within(point: (u16, u16), corner_ul: (u16, u16), corner_lr: (u16, u16)) -> bool {
        point.0 >= corner_ul.0 && point.0 <= corner_lr.0 && point.1 >= corner_ul.1 &&
        point.1 <= corner_lr.1
    }

    fn pos_is_okay(pos: (u16, u16), existing_hero: &[Target], existing_evil: &[Target]) -> bool {
        let score_ul = (0, 0);
        let score_lr = (SSDisplay::get_width(), SSDisplay::get_height());
        let timer_ul = (constants::DISPLAY_SIZE.0 - SSDisplay::get_width(), 0);
        let timer_lr = (timer_ul.0 + SSDisplay::get_width(), SSDisplay::get_height());
        if Self::point_is_within(pos, score_ul, score_lr) ||
           Self::point_is_within((pos.0 + constants::TARGET_SIZE_50.0, pos.1),
                                 timer_ul,
                                 timer_lr) {
            return false;
        }
        for hero in existing_hero {
            if Self::are_overlapping_targets(hero, pos) {
                return false;
            }
        }
        for evil in existing_evil {
            if Self::are_overlapping_targets(evil, pos) {
                return false;
            }
        }
        true
    }
}

pub struct Target {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub bounty: u16,
    pub birthday: usize,
    pub lifetime: usize,
}

impl Target {
    pub fn new(x: u16,
               y: u16,
               width: u16,
               height: u16,
               bounty: u16,
               birthday: usize,
               lifetime: usize)
               -> Self {
        Target {
            x: x,
            y: y,
            width: width,
            height: height,
            bounty: bounty,
            birthday: birthday,
            lifetime: lifetime,
        }
    }

    fn coord_is_inside(&mut self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn check_for_hit(targets: &mut [Target], touches: &[(u16, u16)]) -> Vec<usize> {
        let mut indices: Vec<usize> = Vec::new();
        for (i, target) in targets.iter_mut().enumerate() {
            for touch in touches {
                if target.coord_is_inside(touch.0, touch.1) {
                    indices.push(i);
                }
            }
        }
        indices
    }
}

