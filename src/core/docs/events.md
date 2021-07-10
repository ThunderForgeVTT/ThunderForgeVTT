# Events

## World Events


- Should have an event code
- Should have world uuid
- Should take optional parameter of Token Events.
- Should take optional parameter of Scene Events.
- Should take optional parameter of Effect Events.
- Should take optional parameter of Audio Events. 
  
## Token Events

- Should contain positional updates
- Should contain ownership boolean
- Should have uuid of token
- Should have linked actor uuid
- Should have optional u32 of z height

## Scene Events

- Should take uuid of scene
- Should take string of scene name
- Should take string of navigation name
- Should have boolean of refresh 
- Should take u16 of light level

- Should take optional parameter Grid Event

### Grid Event

- Should take u32 of grid x scale
- Should take u32 of grid y scale
- Should have boolean of if tiled or not
- Should have optional u32 of x tiles
- Should have optional u32 of y tiles
- Should have optional u32 of z max height
