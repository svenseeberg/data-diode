#include <Wire.h>
#include <hd44780.h>                       // main hd44780 header
#include <hd44780ioClass/hd44780_I2Cexp.h> // i2c expander i/o class header

hd44780_I2Cexp lcd; // declare lcd object: auto locate & config exapander chip

// LCD geometry
const int LCD_COLS = 16;
const int LCD_ROWS = 2;

char incomingByte = ' ';
int n = 0;
int l = 1;
unsigned long counter = 0;
void setup()
{
  int status;
  
  status = lcd.begin(LCD_COLS, LCD_ROWS);
  if(status)
  {
    status = -status;
    hd44780::fatalError(status);
  }
  Serial.begin(9600);
  lcd.print("Idle");
}

void loop() {
  if (Serial.available() > 0) {
    incomingByte = Serial.read();
    Serial.println(incomingByte);
    counter++;
    if(incomingByte == char(1)) {
      lcd.setCursor(0,0);
      lcd.write("Transfer");
    } else if(incomingByte == char(4)) {
      lcd.clear();
      lcd.setCursor(0,0);
      lcd.write("Idle");
    }
    lcd.setCursor(0,1);
    lcd.print(counter, DEC);
    lcd.write(" B");
  }
}
