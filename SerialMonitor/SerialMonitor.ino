#include <Wire.h>
#include <hd44780.h>                       // main hd44780 header
#include <hd44780ioClass/hd44780_I2Cexp.h> // i2c expander i/o class header

hd44780_I2Cexp lcd; // declare lcd object: auto locate & config exapander chip

// LCD geometry
const int LCD_COLS = 16;
const int LCD_ROWS = 2;

int line = 0;
int pos = 0;

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
  lcd.print("Booting ...");
}

void loop() {

}

void toggle_line() {
  if (line == 0) {
    line = 1;
  } else {
    line = 0;
  }
  lcd.setCursor(line,0);
}

void serialEvent() {
  while (Serial.available()) {
    char inChar = (char)Serial.read();
    if (inChar == char('\n')) {
      toggle_line();
    } else {
      lcd.write(inChar);
    }
  }
}
