# Host code

This directory contains code that targets the A53 cores (the APU).

## Notes

### How to set up wifi

(because I always forget)

``` console
$ # turn on the wifi interface (if necessary)
$ ifconfig wlan0 up

$ # enter credentials (ssid / password) here
$ vi /etc/wpa_supplicant.conf
$ tail -n6 /etc/wpa_supplicant.conf
network={
        ssid="MyAwesomeAccessPoint"
        scan_ssid=1
        key_mgmt=WPA-PSK
        psk="SuperSecretPassword!"
}

$ # run wpa_supplicant to connect to the access point
$ wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf

$ # ssid should appear here
$ iwconfig
wlan0     IEEE 802.11  ESSID:"MyAwesomeAccessPoint"
          Mode:Managed  Frequency:2.412 GHz  Access Point: 78:44:76:D9:6A:80
          Bit Rate=150 Mb/s   Tx-Power=20 dBm
          Retry short limit:7   RTS thr:off   Fragment thr:off
          Encryption key:off
          Power Management:on
          Link Quality=70/70  Signal level=-1 dBm
          Rx invalid nwid:0  Rx invalid crypt:0  Rx invalid frag:0
          Tx excessive retries:0  Invalid misc:0   Missed beacon:0

$ # get an IP address
$ udhcpc -i wlan0

$ # print the wlan address of the device
$ ip addr
(..)
3: wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500 qdisc mq state UP group default qlen 1000
    link/ether 20:c3:8f:89:c6:a8 brd ff:ff:ff:ff:ff:ff
    inet 192.168.1.13/24 brd 192.168.1.255 scope global wlan0
       valid_lft forever preferred_lft forever
(..)

$ # this should now work
$ ping -c3 github.com
```
