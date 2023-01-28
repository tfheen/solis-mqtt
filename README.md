# solis-mqtt

This is a tool that connects to a Solis 5G inverter's Wifi stick
outlet using an RS485 adaptor.  Quite similar to the [setup
here](https://www.briandorey.com/post/solar-upgrade-solis-1-5kw-inverter-raspberry-pi-rs485-logging),
but in Rust instead of Python, and picking out much more of the data.

You will probably want to change the MQTT endpoint, unless your MQTT
server is named "mqtt".
