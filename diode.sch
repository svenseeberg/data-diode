EESchema Schematic File Version 4
EELAYER 30 0
EELAYER END
$Descr A4 11693 8268
encoding utf-8
Sheet 1 1
Title ""
Date ""
Rev ""
Comp ""
Comment1 ""
Comment2 ""
Comment3 ""
Comment4 ""
$EndDescr
$Comp
L Connector:Raspberry_Pi_2_3 J?
U 1 1 60A3D842
P 5000 6000
F 0 "J?" H 5000 7481 50  0000 C CNN
F 1 "Raspberry_Pi_2_3" H 5000 7390 50  0000 C CNN
F 2 "" H 5000 6000 50  0001 C CNN
F 3 "https://www.raspberrypi.org/documentation/hardware/raspberrypi/schematics/rpi_SCH_3bplus_1p0_reduced.pdf" H 5000 6000 50  0001 C CNN
	1    5000 6000
	1    0    0    -1  
$EndComp
$Comp
L MCU_Module:Arduino_Nano_v2.x A?
U 1 1 60A47158
P 1900 4250
F 0 "A?" H 1900 3069 50  0000 C CNN
F 1 "Arduino_Nano_v2.x" H 1900 3160 50  0000 C CNN
F 2 "Module:Arduino_Nano" H 1900 4250 50  0001 C CIN
F 3 "https://www.arduino.cc/en/uploads/Main/ArduinoNanoManual23.pdf" H 1900 4250 50  0001 C CNN
	1    1900 4250
	-1   0    0    1   
$EndComp
$Comp
L Connector:Raspberry_Pi_2_3 J?
U 1 1 60A3F3D7
P 5000 3100
F 0 "J?" H 5000 4581 50  0000 C CNN
F 1 "Raspberry_Pi_2_3" H 5000 4490 50  0000 C CNN
F 2 "" H 5000 3100 50  0001 C CNN
F 3 "https://www.raspberrypi.org/documentation/hardware/raspberrypi/schematics/rpi_SCH_3bplus_1p0_reduced.pdf" H 5000 3100 50  0001 C CNN
	1    5000 3100
	1    0    0    -1  
$EndComp
$Comp
L pspice:DIODE D?
U 1 1 60A68CD7
P 3300 3500
F 0 "D?" V 3254 3628 50  0000 L CNN
F 1 "DIODE" V 3345 3628 50  0000 L CNN
F 2 "" H 3300 3500 50  0001 C CNN
F 3 "~" H 3300 3500 50  0001 C CNN
	1    3300 3500
	0    1    1    0   
$EndComp
$Comp
L Device:R R1
U 1 1 60A69A22
P 2900 4850
F 0 "R1" V 2693 4850 50  0000 C CNN
F 1 "1k" V 2784 4850 50  0000 C CNN
F 2 "" V 2830 4850 50  0001 C CNN
F 3 "~" H 2900 4850 50  0001 C CNN
	1    2900 4850
	0    1    1    0   
$EndComp
Wire Wire Line
	4200 2200 3300 2200
Wire Wire Line
	3300 2200 3300 3300
Wire Wire Line
	3300 4850 3050 4850
Wire Wire Line
	2400 4850 2750 4850
Wire Wire Line
	3300 3700 3300 4850
Connection ~ 3300 4850
Wire Wire Line
	4200 5200 3300 5200
Wire Wire Line
	3300 5200 3300 4850
$Comp
L power:GND #PWR?
U 1 1 60A6F615
P 2600 2200
F 0 "#PWR?" H 2600 1950 50  0001 C CNN
F 1 "GND" H 2605 2027 50  0000 C CNN
F 2 "" H 2600 2200 50  0001 C CNN
F 3 "" H 2600 2200 50  0001 C CNN
	1    2600 2200
	1    0    0    -1  
$EndComp
$Comp
L Device:R R2
U 1 1 60A6FF9B
P 2950 2200
F 0 "R2" V 2743 2200 50  0000 C CNN
F 1 "10k" V 2834 2200 50  0000 C CNN
F 2 "" V 2880 2200 50  0001 C CNN
F 3 "~" H 2950 2200 50  0001 C CNN
	1    2950 2200
	0    1    1    0   
$EndComp
Connection ~ 3300 2200
Wire Wire Line
	3300 2200 3100 2200
Wire Wire Line
	2600 2200 2800 2200
$EndSCHEMATC
