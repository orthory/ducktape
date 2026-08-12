// Ice composition around an application-owned typed DataGrid extern.
// Rows, column keys, sort policy, and edit values stay in Rust. Give the slot
// bounded width and height outside any scrolling ancestor; the grid owns both
// native scroll axes.

component DataGrid.Frame(title:str, description:str, rows:i64, columns:i64)
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
          text rows @meta
          text "rows" @caption
          text "×" @caption
          text columns @meta
          text "columns" @caption
      slot
