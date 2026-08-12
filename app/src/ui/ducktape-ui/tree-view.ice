// Ice composition around an application-owned typed TreeView extern.
// Hierarchy, retained expansion, lazy loading, and rename state stay in Rust.
// Give the slot a bounded height outside any vertically scrolling ancestor;
// the retained TreeView owns vertical scrolling.

component TreeView.Frame(title:str, description:str, count:i64)
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
          text "nodes" @caption
      slot
