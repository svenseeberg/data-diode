#include <Wire.h>
#include <hd44780.h>                       // main hd44780 header
#include <hd44780ioClass/hd44780_I2Cexp.h> // i2c expander i/o class header

hd44780_I2Cexp lcd; // declare lcd object: auto locate & config exapander chip

// LCD geometry
const int LCD_COLS = 16;
const int LCD_ROWS = 2;

int x = 0;
unsigned long counter = 0;
unsigned int f_counter = 0;
unsigned int f_counter_prev = 0;
bool transfer = false;
bool transfer_prev = false;
bool update = false;

void setup()
{
  int status;
  
  status = lcd.begin(LCD_COLS, LCD_ROWS);
  if(status)
  {
    status = -status;
    hd44780::fatalError(status);
  }
  Serial.begin(57600);
  lcd.print("Idle");
  lcd.setCursor(0,1);
  lcd.print(counter, DEC);
  lcd.write("B");
  lcd.setCursor(9,0);
  lcd.print(f_counter, DEC);
  lcd.write("F");
}

void loop() {
  x++;
  if (x>=10) {
    x = 0;
    if(transfer and not transfer_prev) {
      lcd.setCursor(0,0);
      lcd.write("Transfer");
    } else if (not transfer and transfer_prev) {
      lcd.setCursor(0,0);
      lcd.write("Idle    ");
    }
    if (f_counter != f_counter_prev) {
      lcd.setCursor(9,0);
      lcd.print(f_counter, DEC);
      lcd.write("F");
    }
    if (update) {
      update = false;
      lcd.setCursor(0,1);
      lcd.print(counter, DEC);
      lcd.write("B");
    }
  }
}

void serialEvent() {
  update = true;
  while (Serial.available()) {
    char inChar = (char)Serial.read();
    counter++;
    if (inChar == char(1)) {
      transfer = true;
      f_counter++;
    } else if (inChar == char(4)) {
      transfer = false;
    }
  }
}
