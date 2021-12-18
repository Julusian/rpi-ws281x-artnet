use artnet_protocol::ArtCommand;
use artnet_protocol::PortAddress;
use rs_ws281x::RawColor;
use rs_ws281x::{ChannelBuilder, ControllerBuilder, StripType};
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

// TODO - these should be configurable
const BYTES_PER_PIXEL: usize = 3;
const PIXELS_PER_UNIVERSE: usize = 170;
const UNIVERSE_COUNT: u8 = 3;

const EMPTY_COLOR: RawColor = [0, 0, 0, 0];

#[derive(Clone)]
struct PixelData {
    pub changed: bool,
    pub brightness: u8,
    pub pixels: Vec<RawColor>,
}
impl Default for PixelData {
    fn default() -> Self {
        PixelData {
            changed: true,
            brightness: 0,
            pixels: vec![],
        }
    }
}

fn start_artnet_thread(shared_data: Arc<Mutex<PixelData>>) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        let socket = UdpSocket::bind(("0.0.0.0", 6454)).expect("Could not bind socket for artnet");
        match socket.set_nonblocking(true) {
            Ok(_) => println!("Activated non-blocking mode"),
            Err(e) => println!("Could not activate non-blocking mode: {}", e),
        };
        println!("Bound socket for artnet");

        let mut start = Instant::now();
        let mut frames = 0;
        loop {
            let mut buffer = [0u8; 1024];
            if let Ok((length, _addr)) = socket.recv_from(&mut buffer) {
                let command = ArtCommand::from_buffer(&buffer[..length])
                    .expect("Could not parse artnet command");

                // println!("Received artnet data: {:?}", command);
                if let ArtCommand::Output(output) = command {
                    frames += 1;
                    if start.elapsed().as_secs() >= 1 {
                        println!("{} fps", frames);
                        start = Instant::now();
                        frames = 0;
                    }

                    if output.port_address >= PortAddress::from(1)
                        && output.port_address <= PortAddress::from(UNIVERSE_COUNT)
                    {
                        let mut pixel_offset = 0;
                        for n in 1..UNIVERSE_COUNT {
                            // Mmmm pretty...
                            if output.port_address == PortAddress::from(n) {
                                pixel_offset = PIXELS_PER_UNIVERSE * ((n as usize) - 1);
                            }
                        }
                        let raw_data = output.data.as_ref();

                        let new_brightness = if output.port_address == PortAddress::from(1)
                            && raw_data.len() == 512
                        {
                            raw_data.last().cloned()
                        } else {
                            None
                        };

                        let mut new_pixels = vec![EMPTY_COLOR; PIXELS_PER_UNIVERSE];
                        for i in 0..(PIXELS_PER_UNIVERSE - 1) {
                            let o = i * BYTES_PER_PIXEL;
                            if o >= raw_data.len() || (o + BYTES_PER_PIXEL) > raw_data.len() {
                                // Will overflow the source array
                                break;
                            }

                            // Note: only supports RGB for now, no A
                            new_pixels[i] = [raw_data[o], raw_data[o + 1], raw_data[o + 2], 0];
                        }

                        // println!("new_pixels={}, offset={}", new_pixels.len(), pixel_offset);

                        {
                            // Update the output data
                            let mut locked = shared_data.lock().unwrap();
                            (*locked).changed = true;

                            if let Some(brightness) = new_brightness {
                                (*locked).brightness = brightness;
                            }

                            let pixels_vec = &mut (*locked).pixels;
                            {
                                // Not optimal, but rare enough I dont care
                                while pixels_vec.len() < pixel_offset + PIXELS_PER_UNIVERSE {
                                    pixels_vec.push(EMPTY_COLOR);
                                }
                            }

                            for (i, v) in new_pixels.into_iter().enumerate() {
                                pixels_vec[pixel_offset + i] = v;
                            }
                        }
                    }
                }
            } else {
                thread::sleep(Duration::from_millis(1));
            }
        }
    })
}

fn start_ws281x_thread(shared_data: Arc<Mutex<PixelData>>) -> std::thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut controller = ControllerBuilder::new()
            .freq(800_000)
            .dma(10)
            .channel(
                0, // Channel Index
                ChannelBuilder::new()
                    .pin(12)
                    .count(PIXELS_PER_UNIVERSE as i32 * UNIVERSE_COUNT as i32)
                    .strip_type(StripType::Ws2811Rgb)
                    .brightness(255)
                    .build(),
            )
            // TODO - in future this could have a second channel added
            .build()
            .unwrap();

        let mut next_data: Option<PixelData>;
        loop {
            {
                // Get a copy of the new data to render
                let mut locked = shared_data.lock().unwrap();
                if (*locked).changed {
                    // take a copy
                    next_data = Some((*locked).clone());

                    // clear the changed flag
                    (*locked).changed = false;
                } else {
                    next_data = None;
                }
            }

            if let Some(data) = next_data {
                controller.set_brightness(0, data.brightness);

                let leds = controller.leds_mut(0);
                let count = std::cmp::min(leds.len(), data.pixels.len());
                for i in 0..count {
                    // copy over pixel data
                    leds[i] = data.pixels[i];
                }

                controller.render().unwrap();
                controller.wait().unwrap();
                // TODO - keep to target fps
            } else {
                // TODO - keep to target fps
                thread::sleep(Duration::from_millis(20));
            }
        }
    })
}

fn main() {
    println!("Starting!");

    let shared_data = Arc::new(Mutex::new(PixelData::default()));

    let artnet_thread = start_artnet_thread(shared_data.clone());
    let ws281x_thread = start_ws281x_thread(shared_data.clone());

    println!("Ready");
    ws281x_thread.join().unwrap();
    artnet_thread.join().unwrap();
    // TODO - can we abort whenever either thread panics?
}
