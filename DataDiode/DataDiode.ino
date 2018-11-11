#include <Wire.h>
#include <hd44780.h>                       // main hd44780 header
#include <hd44780ioClass/hd44780_I2Cexp.h> // i2c expander i/o class header

hd44780_I2Cexp lcd; // declare lcd object: auto locate & config exapander chip

// LCD geometry
const int LCD_COLS = 16;
const int LCD_ROWS = 2;

char incomingByte = ' ';
int x = 0;
unsigned long counter = 0;
unsigned int f_counter = 0;
void setup()
{
  int status;
  
  status = lcd.begin(LCD_COLS, LCD_ROWS);
  if(status)
  {
    status = -status;
    hd44780::fatalError(status);
  }
  Serial.begin(19200);
  lcd.print("Idle");
}

void loop() {
  if (Serial.available() > 0) {
    incomingByte = Serial.read();
    Serial.println(incomingByte);
    counter++;
    x++;
    if(incomingByte == char(1)) {
      lcd.setCursor(0,0);
      lcd.write("Transfer");
      lcd.setCursor(9,0);
      f_counter++;
      lcd.print(f_counter, DEC);
      lcd.write("F");
    } else if(incomingByte == char(4)) {
      lcd.setCursor(0,0);
      lcd.write("Idle    ");
      lcd.setCursor(0,1);
      lcd.print(counter, DEC);
      lcd.write("B");
    } else if (x >= 10) {
      x = 0;
      lcd.setCursor(0,1);
      lcd.print(counter, DEC);
      lcd.write("B");
    }
  }
}
