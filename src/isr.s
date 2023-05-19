MOV  R0, #32  ;Init the character
.loop
SWI  WriteC  ;Print it
ADD  R0, R0, #1 ;Increment it     // 1
CMP  R0, #126 ;Check the limit    // 1 can remove using adds (something flag)
BLE  loop  ;Loop if not finished
