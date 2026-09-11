// Ice composition around an application-owned typed LogTimeline extern.
// The fixed-height append stream, retained state, and row data remain in Rust;
// Ice owns the surrounding product layout and event routes.

component LogTimeline.Frame(title:str, description:str)
  box #root r=11.0 @panel
    col w=fill gap=12.0
      col w=fill gap=4.0
        text title @section_title
        text description @caption
      slot
