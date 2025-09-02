# rvban

Implementation of the VBAN protocol by Vincent Burel ([https://vb-audio.com/Voicemeeter/vban.htm](https://vb-audio.com/Voicemeeter/vban.htm)) written in pure Rust

_Work in progress - feedback appreciated!_

This repository contains the code for two binaries, one of which streams your system's audio to a peer and the other acts as a sink for incoming VBAN streams. 

## System requirements

- Linux system
- [ALSA](https://www.alsa-project.org/wiki/Main_Page) installed (I use it in combination with pipewire)

I developed this application for my Raspberry Pi to running Moode Audio to support VBAN. So I can confirm this runs on a RPi 4 with 4 GB RAM. 


# Binaries

## vban_sink

### Usage

Start a VBAN stream, for example by using the Voicemeeter application from the creator of VBAN (vb-audio.com). Direct the outgoing stream to the machine that should run vban_sink. Run `vban_sink` (simple as that). Make sure port 6980 is open for incoming udp packets. vban_sink adapts to the incoming sample rate. __Only 16 bit format supported, though!.__

### Options

- -p : Specify a different port (other that 6980)
- -c : Work in progress - _not supported yet_. 
- -s : Specify a stream name if you only want to accept one specific stream. 
- -x : Prepend silence when starting playback. This is useful to avoid buffer underrun on instable networks.
- -d : Audio device name to be used as sink. Default is 'default' which usually points to the default audio device when using ALSA.
- -m : Execute a script on playback state change.
- -l : Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3)
- -r : Sample rate
- -h : Print help

### Executing a script on playback state change

If the option `-m` is used a script may be executed on playback state change. The script will be invoked with the argmuents "playback_started" or "playback_stopped" respectiely. 


## vban_source

### Usage

Play audio through any application on your system. Start the vban_source by invoking `vban_source -i <IP-address>` with the IP address of the receiving system. If you want to reduce data throughput of your network, you may use the Opus enccoder by using the parameter `-e opus`, when invokung vban_source.

### Options

- -i : IP address of the receiver, e.g. 192.168.0.100
- -p : Port of the receiver. Specify a port if you don't want to use the default port 6980
- -l : Specify an IP-address if you don't want to bind to all interfaces
- -o : Specify a different port if you don't want to use port 6980
- -c : Use a config file
- -s : Specify a stream name (defaults to Stream1)
- -d : Name of the audio device that is used as a source (default is "default")
- -e : Encoder (Opus, PCM)
- -v : Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3)
- -h : Print help
