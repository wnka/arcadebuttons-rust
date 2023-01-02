arcadebuttons-rust
=====================

Reads the input of buttons from GPIO pins on a Raspberry Pi and displays them on a webpage using [warp](https://github.com/seanmonstar/warp) and WebSockets.

https://user-images.githubusercontent.com/92748/210278032-49063cc1-6c6f-4bc5-b535-5d612d524e11.mp4

# Node.js version

I originally wrote this in Node.js. It was fun to rewrite it in Rust and learn some of these libraries. The Node.js version is [here](https://github.com/wnka/arcadebuttons-node-pi).

# Backstory

I play arcade games (mostly shmups) and sometimes stream on [twitch.tv](http://twitch.tv/pdp80) from my arcade cabinet.  My JAMMA capture setup is [detailed here](http://shmups.system11.org/viewtopic.php?f=6&t=45917). I tried taping a webcam to my ceiling to capture joystick movements, but its hard to tell what inputs are being made due to poor lighting and my hands being in the way. Other players who play from within MAME can have a nice input display that shows the movements they're making. This project is an attempt at providing that kind of input display for an actual arcade cabinet.

I have a Raspberry Pi under the control panel of my arcade cabinet with the control wires split off and plugged into the GPIO pins. This app provides a nice display that I can use while streaming by just including the webpage in OBS. It works!

# systemd script to start on boot

Tested on a Raspberry Pi. You might need to change the `User` and path in `ExecStart`.

```
[Unit]
Description=Buttons Input Display
After=network.target
StartLimitIntervalSec=0

[Service]
Type=simple
Restart=always
RestartSec=1
User=pi
ExecStart=/usr/bin/env /home/pi/bin/arcadebuttons

[Install]
WantedBy=multi-user.target
```

# Note

[This thread on Twitter](https://twitter.com/wnka/status/1384361401301360647) discusses how one user needed to convert the 5v from the arcade inputs down to 3.3v for the Pi. Your mileage may vary, I've never needed to do this but you might!
