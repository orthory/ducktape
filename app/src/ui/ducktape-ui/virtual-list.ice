// Ice composition around an application-owned typed VirtualList extern.
// The retained list state, item type, and event reducer stay in Rust.
// Give the slot a bounded height outside any vertically scrolling ancestor;
// the retained VirtualList owns vertical scrolling.

component VirtualList.Frame(title:str, description:str, count:i64)
  box #root r=11.0 @panel
    col w=fill gap=12.0
      row
        with
          w=fill
          gap=12.0
          align=center
        col w=fill gap=4.0
          text title @section_title
          text description @caption
        row gap=4.0 align=center
          text count @meta
          text "items" @caption
      slot
