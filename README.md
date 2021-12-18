
### Hardware setup

This has only been tested on a 3B. More recent models may work, but may need code changes to use the correct GPIO pins.
Note: It is compiled for arm7, this can likely be changed without issue

Connect up the data line and ground of the lights to the pi for PWM mode as descibed [here](https://github.com/jgarff/rpi_ws281x#gpio-usage).  
The application is expecting GPIO12, so pin 32 should be used (pin 30 is conveniently a GND).

### Compiling

This assumes you are on linux, to compile from other platforms the steps will need adjusting

* Install Rust from [rustup.rs](https://rustup.rs/)
* Add the arm7 target `rustup target add armv7-unknown-linux-gnueabihf`
* Install required libs `sudo apt install libclang-dev gcc-arm-linux-gnueabihf`
* Build it `cargo build --release`
* The resulting binary `./target/armv7-unknown-linux-gnueabihf/release/pi-ws281x-artnet` can be copied to the pi

### Installing

Copy the binary to the raspberry pi

* Create the file `/etc/systemd/system/ws281x.service`
```
[Unit]
Description=WS281x Artnet
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/home/pi
ExecStart=/home/pi/pi-ws281x-artnet
Restart=on-failure
KillSignal=SIGINT
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
```
* `sudo systemctl daemon-reload`
* `sudo systemctl enable ws281x.service`
* `sudo systemctl start ws281x.service`