//#![windows_subsystem = "windows"]
#[cfg(target_pointer_width = "64")]
extern crate native_windows_gui as nwg;
use std::rc::Rc;
use std::thread;
use std::time::Duration;
use std::io::Write;
use string_builder:: Builder;
use std::sync::mpsc::{channel, Sender, Receiver};
use stretch::style::Dimension;
use stretch::geometry::Size;
static TAG_COM   : &str = "COM";
static TAG_NONE  : &str = "None";
static TAG_WRITE : &str = "<\n\\W>";
static TAG_READ  : &str = "<\n\\R>";
static TAG_SETTINGS  : &str = "<\n\\S>";
static TAG_ERR   : &str = "<\n\\E>";
static TAG_WRITE_SUCCESS : &str = "<\n\\w>";

static TAG_SENT  : &str = "Portions sent:";
                      
fn get_avail_ports() -> Vec<String>
{
    let mut avail_ports = serialport::available_ports();
    let mut avail_port_names = Vec::new();
    match avail_ports
    {
        Ok(ref mut ports) =>
        {
            for port in ports {
                if serialport::new(&port.port_name, 9_600).open().is_ok(){ avail_port_names.push(port.port_name.clone()); }
            }
        },
        _ => {},
    }
    avail_port_names.sort();

    return avail_port_names;
}
                        
fn next_port(name: &str) -> String
{
    let (_, num_str) = name.split_at(TAG_COM.len());
    let num = num_str.parse::<i32>().unwrap() + 1;
    let mut bld = Builder::default();
    bld.append(TAG_COM);
    bld.append(num.to_string());
    return bld.string().unwrap();
}

fn prev_port(name: &str) -> String 
{
    let (_, num_str) = name.split_at(TAG_COM.len());
    let num = num_str.parse::<i32>().unwrap() - 1;
    let mut bld = Builder::default();
    bld.append(TAG_COM);
    if num >= 0 {
        bld.append(num.to_string());
    }
    return bld.string().unwrap();
}

fn send_error(
 tx: &Sender<Vec<u8>>,
 err_msg: &str,
){
        let mut bld = Builder::default();
        bld.append(TAG_ERR);
        bld.append(err_msg);
        _ = tx.send(bld.string().unwrap().as_bytes().to_vec()); 
}

fn send_settings(
 tx: &Sender<Vec<u8>>,
 port: &Box<dyn serialport::SerialPort>,
){

        let mut bld = Builder::default();
        bld.append(TAG_SETTINGS);
        bld.append("Baudwidth:");
        bld.append(port.baud_rate().unwrap().to_string());
        bld.append("\n");
        bld.append("Databits:");
        match port.data_bits()
        {
            Ok(t) => { bld.append(u8::from(t).to_string()); },
            _ => bld.append("error"),
        }
        bld.append("\n");
        bld.append("Parity:");

        match port.parity()
        {
            Ok(t) => 
            {
                use serialport::Parity as spp;
                match t
                {
                    spp::None  => bld.append("None"),
                    spp::Odd   => bld.append("Odd"),
                    spp::Even  => bld.append("Even"),
                }
            },
            _ => bld.append("error"),
        }
        bld.append("\n");
        bld.append("Control:");

        match port.flow_control()
        {
            Ok(t) => 
            {
                use serialport::FlowControl as spfc;
                match t
                {
                    spfc::None      => bld.append("None"),
                    spfc::Software  => bld.append("XON/XOFF"),
                    spfc::Hardware  => bld.append("RTS/CTS"),
                }
            },
            _ => bld.append("error"),
        }
        bld.append("\n");
        bld.append("Stop Bits:");

        match port.stop_bits()
        {
            Ok(t) => 
            {
                use serialport::StopBits as spsb;
                match t
                {
                    spsb::One       => bld.append("1"),
                    spsb::Two       => bld.append("2"),
                }
            },
            _ => bld.append("error"),
        }
        bld.append("\n");
        bld.append("Timeout:");
        bld.append(port.timeout().as_millis().to_string());
        bld.append("ms");

        _ = tx.send(bld.string().unwrap().as_bytes().to_vec()); 
}

fn port_worker(
    tx: Sender<Vec<u8>>,
    rx: Receiver<String>,
)
{
    
    let mut port_user_write  : Option<Box<dyn serialport::SerialPort>> = None;
    let mut port_user_read   : Option<Box<dyn serialport::SerialPort>> = None;

    loop{
        match port_user_read
        {
            Some(ref mut port) =>
            {
                if port.bytes_to_read().unwrap() > 0
                {
                    let mut serial_buf = vec![0; 1024];
                    match port.read(serial_buf.as_mut_slice())
                    {
                        Ok(_) => { _ = tx.send(serial_buf); },
                        _ =>     { send_error(&tx, "ERROR: failed to read"); },
                    }
                }
            },
            _ => {},
        } 
        match rx.try_recv()
        {
            Ok(text) => 
            {
                let mut msg_type = 'M';
                if text.starts_with(TAG_READ)  == true { msg_type = 'R'; }
                if text.starts_with(TAG_WRITE) == true { msg_type = 'W'; }

                match msg_type
                {
                    'W' =>
                    {
                        let mut bool_need_to_open = false;
                        let (_, name) = text.split_at(TAG_WRITE.len());
                        if name == TAG_NONE { port_user_write = None; }
                        else{
                            match port_user_write
                            {
                                Some(ref port) => 
                                {
                                    if name != port.name().unwrap() { bool_need_to_open = true; }
                                },
                                _ => { bool_need_to_open = true; },
                            }
                            
                        }
                        if bool_need_to_open == true
                        {
                            match serialport::new(name, 9_600)
                                             .timeout(Duration::from_millis(10))
                                             .open()
                            {
                                Ok(port) =>
                                {
                                    send_settings(&tx, &port);
                                    let name = port.name().unwrap();
                                    port_user_write = Some(port);

                                    let mut bld = Builder::default();
                                    bld.append(TAG_WRITE);
                                    bld.append(name);
                                    _ = tx.send(bld.string().unwrap().as_bytes().to_vec());
                                },
                                _ => { send_error(&tx, "Failed to open port."); },
                            }
                        
                        }
                    },
                    'R' =>
                    {
                        let mut bool_need_to_open = false;
                        let (_, name) = text.split_at(TAG_READ.len());
                        if name == TAG_NONE { port_user_read = None; }
                        else{
                            match port_user_read
                            {
                                Some(ref port) => 
                                {
                                    if name != port.name().unwrap() { bool_need_to_open = true; }
                                },
                                _ => { bool_need_to_open = true; },
                            }
                            
                        }
                        if bool_need_to_open == true
                        {
                            match serialport::new(name, 9_600)
                                             .timeout(Duration::from_millis(10))
                                             .open()
                            {
                                Ok(port) =>
                                {
                                    send_settings(&tx, &port);
                                    let name = port.name().unwrap();
                                    port_user_read = Some(port);

                                    let mut bld = Builder::default();
                                    bld.append(TAG_READ);
                                    bld.append(name);
                                    _ = tx.send(bld.string().unwrap().as_bytes().to_vec());
                                },
                                _ => { send_error(&tx, "Failed to open port."); },
                            }
                        
                        }
                    },
                    _ =>
                    {
                        match port_user_write
                        {
                            Some(ref mut port) =>
                            {
                                match port.write(text.as_bytes())
                                {
                                    Ok(_) => 
                                    {
                                        let mut bld = Builder::default();
                                        bld.append(TAG_WRITE_SUCCESS);
                                        bld.append(1);
                                        _ = tx.send(bld.string().unwrap().as_bytes().to_vec());
                                    },
                                    _ => { send_error(&tx, "Failed to write."); },
                                }
                            },
                            _ => { send_error(&tx, "Port for writing is invalid");   }
                        }                      
                    },                    
                }
            },
            _ =>  {},
        }
    }
}

fn main()
{    
    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");

    let mut textbox_settings    = Default::default();
    let mut ddlist_w            = Default::default();
    let mut ddlist_r            = Default::default();

    let mut label_state         = Default::default();
    let mut label_settings      = Default::default();
    let mut label_sent          = Default::default();
    let mut label_output        = Default::default();
    let mut label_input         = Default::default();
    let mut label_settings_write = Default::default();
    let mut label_settings_read  = Default::default();

    let mut text_packages_sent     = Default::default();
    
    let mut field_input         = Default::default();

    let mut window_main         = Default::default();
    let mut field_output        = Default::default();    
    let     layout              = Default::default();

    let mut div_input           = Default::default();
    let mut div_output          = Default::default();
    
    let mut div_state           = Default::default();
    let mut div_state_col       = Default::default();
    let mut div_state_row       = Default::default();

    let mut div_settings        = Default::default();
    let mut div_settings_w      = Default::default();
    let mut div_settings_r      = Default::default();

    let layout_input            = Default::default();
    let layout_output           = Default::default();
    let layout_state            = Default::default();
    let layout_state_col        = Default::default();  
    let layout_state_row        = Default::default();  
    let layout_settings         = Default::default();
    let layout_settings_write   = Default::default();    
    let layout_settings_read    = Default::default();

    let mut button_settings_w     = Default::default();
    let mut button_settings_r     = Default::default();

    let avail_port_names = get_avail_ports();
/*=====================DIVs=========================*/

    nwg::Window::builder()
                .size((920, 600))
                .position((0, 0))
                .title("Lab1")
                .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
                .build(&mut window_main)
                .unwrap()
    ;

    let mut font = nwg::Font::default();

    nwg::Font::builder()
        .size(20)
        .family("Segoe UI")
        .weight(500)
        .build(&mut font)
	.unwrap()
    ;
    _ = nwg::Font::set_global_default(Some(font));
    /*==========DIV==============*/

    nwg::Frame::builder()
                .parent(&window_main)
                .build(&mut div_input)
                .unwrap()
    ;

    nwg::Frame::builder()
                .parent(&window_main)
                .build(&mut div_output)
                .unwrap()
    ;

    nwg::Frame::builder()
                .parent(&window_main)
                .build(&mut div_state)
                .unwrap()
    ;

    nwg::Frame::builder()
                .parent(&window_main)
                .build(&mut div_settings)
                .unwrap()
    ;


    nwg::Frame::builder()
                .parent(&div_settings)
                .build(&mut div_settings_w)
                .unwrap()
    ;

    nwg::Frame::builder()
                .parent(&div_settings)
                .build(&mut div_settings_r)
                .unwrap()
    ;


    nwg::Frame::builder()
                .parent(&div_state)
                .build(&mut div_state_row)
                .unwrap()
    ;

    nwg::Frame::builder()
                .parent(&div_state_row)
                .build(&mut div_state_col)
                .unwrap()
    ;
/*=====================Elements=========================*/

    nwg::ComboBox::builder()
                 .parent(& div_settings_r)
                 .build(& mut ddlist_r)
                 .unwrap()
    ;

    nwg::ComboBox::builder()
                 .parent(& div_settings_w)
                 .collection(avail_port_names.clone())
                 .build(& mut ddlist_w)
                 .unwrap()
    ;

    nwg::TextBox::builder()
                 .text("")
                 .readonly(true)
                 .flags(nwg::TextBoxFlags::VISIBLE)
                 .parent(&div_output)
                 .build(&mut field_output)
                 .unwrap()
    ;

    nwg::TextBox::builder()
                 .text("")
                 .flags(nwg::TextBoxFlags::VISIBLE)
                 .readonly(true)
                 .parent(&div_state_row)
                 .build(&mut textbox_settings)
                 .unwrap()
    ;


    nwg::Label::builder()
                 .h_align(nwg::HTextAlign::Center)              
                 .text("State")
                 .parent(&div_state)
                 .build(&mut label_state)
                 .unwrap()
    ;

    nwg::Label::builder()
                 .h_align(nwg::HTextAlign::Center)              
                 .text("Settings")
                 .parent(&div_settings)
                 .build(&mut label_settings)
                 .unwrap()
    ;
                
    nwg::Label::builder()
                 .h_align(nwg::HTextAlign::Center)              
                 .text(TAG_SENT)
                 .parent(&div_state_col)
                 .build(&mut label_sent)
                 .unwrap()
    ;

    nwg::Label::builder()
                 .text("Input")
                 .h_align(nwg::HTextAlign::Center)
                 .parent(&div_input)
                 .build(&mut label_input)
                 .unwrap()
    ;    

    nwg::Label::builder()
                 .text("Output")
                 .h_align(nwg::HTextAlign::Center)
                 .parent(&div_output)
                 .build(&mut label_output)
                 .unwrap()
    ;    
    
    nwg::Label::builder()
                 .h_align(nwg::HTextAlign::Center)              
                 .text("Transmitter")
                 .parent(&div_settings_w)
                 .build(&mut label_settings_write)
                 .unwrap()
    ;

    nwg::Label::builder()
                 .h_align(nwg::HTextAlign::Center)              
                 .text("Receiver")
                 .parent(&div_settings_r)
                 .build(&mut label_settings_read)
                 .unwrap()
    ;

    nwg::TextBox::builder()
                 .text("0")
                 .flags(nwg::TextBoxFlags::VISIBLE)
                 .readonly(true)
                 .focus(true)
                 .parent(&div_state_col)
                 .build(&mut text_packages_sent)
                 .unwrap()
    ;

/*=====================UI elements=========================*/
  
    nwg::TextBox::builder()
                   .parent(&div_input)
                   .flags(nwg::TextBoxFlags::VISIBLE)
                   .build(&mut field_input)
                   .unwrap()
    ;

    nwg::Button::builder()
                .text("Refresh")
                .parent(&div_settings_w)
                .build(& mut button_settings_w)
                .unwrap()
    ;

    nwg::Button::builder()
                .text("Refresh")
                .parent(&div_settings_r)
                .build(& mut button_settings_r)
                .unwrap()
    ;

/*=====================Layouts=========================*/
    nwg::FlexboxLayout::builder()
                    .parent(&div_settings_w)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .child(&label_settings_write)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.1) })
                    .child(&ddlist_w)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.4) })
                    .child(&button_settings_w)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.4) })
                    .build(&layout_settings_write)
                    .unwrap()
    ;

    nwg::FlexboxLayout::builder()
                    .parent(&div_settings_r)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .child(&label_settings_read)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.1) })
                    .child(&ddlist_r)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.4) })
                    .child(&button_settings_r)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.4) })
                    .build(&layout_settings_read)
                    .unwrap()
    ;


    nwg::FlexboxLayout::builder()
                    .parent(&div_input)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .justify_content(stretch::style::JustifyContent::Center)
                    .child(&label_input)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.1) })
                    .child(&field_input)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(1.0) })
                    .auto_spacing(Some(15))
                    .build(&layout_input)
                    .unwrap()
    ;

    nwg::FlexboxLayout::builder()
                    .parent(&div_state_col)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .justify_content(stretch::style::JustifyContent::Center)
                    .child(&label_sent)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.2) })
                    .child(&text_packages_sent)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(0.2) })
                    .build(&layout_state_col)
                    .unwrap()
    ;

    nwg::FlexboxLayout::builder()
                    .parent(&div_state_row)
                    .flex_direction(stretch::style::FlexDirection::Row)
                    .child(&textbox_settings)
                    .child(&div_state_col)
                    .build(&layout_state_row)
                    .unwrap()
    ;


    nwg::FlexboxLayout::builder()
                    .parent(&div_input)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .justify_content(stretch::style::JustifyContent::Center)
                    .child(&label_input)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.1) })
                    .child(&field_input)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(1.0) })
                    .auto_spacing(Some(15))
                    .build(&layout_input)
                    .unwrap()
    ;


    nwg::FlexboxLayout::builder()
                    .parent(&div_output)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .child(&label_output)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.1) })
                    .child(&field_output)
                    .child_size( Size{ width: Dimension::Percent(0.9), height: Dimension::Percent(1.0) })
                    .auto_spacing(Some(15))
                    .build(&layout_output)
                    .unwrap()
    ;                

    nwg::FlexboxLayout::builder()
                    .parent(&div_settings)
                    .justify_content(stretch::style::JustifyContent::Center)
                    .child(&label_settings)
                    .child(&div_settings_w)
                    .child(&div_settings_r)
                    .build(&layout_settings)
                    .unwrap()
    ;                

    nwg::FlexboxLayout::builder()
                    .parent(&div_state)
                    .flex_direction(stretch::style::FlexDirection::Column)
                    .justify_content(stretch::style::JustifyContent::Center)
                    .child(&label_state)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.1) })
                    .child(&div_state_row)
                    .child_size( Size{ width: Dimension::Percent(1.0), height: Dimension::Percent(0.8) })
                    .build(&layout_state)
                    .unwrap()
    ;

    nwg::GridLayout::builder()
                    .parent(&window_main)
                    .child(0, 0, &div_input)
                    .child(1, 0, &div_output)
                    .child(0, 1, &div_state)
                    .child(1, 1, &div_settings)
                    .build(&layout)
                    .unwrap()
    ;


    let (tx, rx_thread) = channel();
    let (tx_thread, rx) = channel();

    let _handle = thread::spawn(move || port_worker(tx_thread, rx_thread));

    let window_main = Rc::new(window_main);
    ddlist_w.set_collection(avail_port_names.clone());
    ddlist_r.set_collection(avail_port_names.clone());
    ddlist_w.push(TAG_NONE.to_string());
    ddlist_r.push(TAG_NONE.to_string());
    let handler = nwg::full_bind_event_handler(&window_main.clone().handle, move |evt, _evt_data, handle| 
    {

        use nwg::Event as E;

        match evt {
            E::OnWindowClose => 
            { 
                nwg::stop_thread_dispatch();
            },
            E::OnComboxBoxSelection =>
            {
                if &handle == &ddlist_w { _ = tx.send(TAG_WRITE.to_string() + &(ddlist_w.selection_string().unwrap())); }
                if &handle == &ddlist_r { _ = tx.send(TAG_READ.to_string()  + &(ddlist_r.selection_string().unwrap())); }            
            },
            E::OnKeyPress =>
            {
                if _evt_data.on_key() == 0x0D
                {
                    _ = tx.send(field_input.text());
                }
            },
            E::OnButtonClick =>
            {

                if &handle == &button_settings_w
                {
                    let mut avail_port_names = get_avail_ports();
                    let current_w_opt = ddlist_w.selection_string();

                    match ddlist_r.selection_string()
                    {
                        Some(ref name) => 
                        {
                            if name != TAG_NONE {
                                let prev = prev_port(&name);
                                match avail_port_names.iter().position(|x| *x == prev)
                                {
                                    Some(idx) => { avail_port_names.remove(idx); },
                                    _ => {},
                                }
                            }
                        },
                        _ => {},
                    }

                    avail_port_names.push(TAG_NONE.to_string());
                    ddlist_w.set_collection(avail_port_names);
       
                    match current_w_opt
                    {
                        Some(ref name) => 
                        {
                            ddlist_w.push(name.clone());
			    ddlist_w.sort();
                            ddlist_w.set_selection_string(&name);
 			}, 
		        _ => {},
		    }

                }
                if &handle == &button_settings_r
                {
                    let mut avail_port_names = get_avail_ports();
                    let current_r_opt = ddlist_r.selection_string();

                    match current_r_opt
                    {
                        Some(ref name) => 
                        {
                            match avail_port_names.iter().position(|x| x == name)
                            {
                                Some(_) => {},
                                _ => { avail_port_names.push(name.clone()); },
                            }
                            ddlist_r.set_selection_string(&name); 
                        },
                        _ => { },
                    }
                    match ddlist_w.selection_string()
                    {
                        Some(ref name) => 
                        {
                            if name != TAG_NONE
                            {
                                let mut index = usize::MAX;
                                let next = next_port(&name);
                                match avail_port_names.iter().position(|x| *x == next)
                                {
                                    Some(idx) => { index = idx; },
                                    _ => {},
                                }
                                if index != usize::MAX { avail_port_names.remove(index); }
                            }
                        },
                        _ => {},
                    }
                    avail_port_names.push(TAG_NONE.to_string());
                    ddlist_r.set_collection(avail_port_names.clone());
                    ddlist_r.sort();
                    match current_r_opt
                    {
                        Some(ref name) => { 
                            let mut index = usize::MAX;
                            match avail_port_names.iter().position(|x| x == name)
                            {
                                Some(idx) => { index = idx; },
                                _ => {},
                            }
                            if index != usize::MAX{ ddlist_r.set_selection_string(&name); }
                        },
                        _ => {},
                    }
                }
        
            },
            _ =>
            {
                match rx.try_recv()
                {
                    Ok(bytes) => 
                    {
                        let mut msg_type = 'M';
                        let text = &String::from_utf8(bytes.to_vec()).expect("Our bytes should be valid utf8");
			if text.starts_with(TAG_WRITE) { msg_type = 'W'; }
                        if text.starts_with(TAG_READ) { msg_type = 'R'; }
                        if text.starts_with(TAG_SETTINGS) { msg_type = 'S'; }
                        if text.starts_with(TAG_WRITE_SUCCESS) { msg_type = 'w'; }
                        if text.starts_with(TAG_ERR) == true { msg_type = 'E'; }
                        match msg_type
                        {
                            'W' =>
                            {   
                                let (_, p_w_name) = text.split_at(TAG_WRITE.len());
                                let mut index = usize::MAX;        
                                match ddlist_r.collection().iter().position(|r| *r == p_w_name)
                                {
                                    Some(idx) => { index = idx; },
				    _ => {},
                                }
                                if index != usize::MAX { ddlist_r.remove(index); }
                                
                                let name_next = next_port(p_w_name);
                                match ddlist_r.collection().iter().position(|r| *r == name_next)
                                {
				    Some(idx) => { index = idx; }
                                    _ => { index = usize::MAX; },
                                }
                                if index != usize::MAX { ddlist_r.remove(index); }
                                
                            },
                            'w' =>
                            {
                                field_input.clear();
                                text_packages_sent.set_text((text_packages_sent.text().parse::<i32>().unwrap() + 1).to_string().as_str());

                            },
                            'R' =>
                            {
                                let (_, p_r_name) = text.split_at(TAG_READ.len());
                                let mut index;

                                match ddlist_w.collection().iter().position(|r| *r == p_r_name)
                                {
                                    Some(idx) => { index = idx; },
                                    _ => { index = usize::MAX; },
                                }
                                if index != usize::MAX { ddlist_w.remove(index); }

                                let name_prev = prev_port(p_r_name);
                                match ddlist_w.collection().iter().position(|r| *r == name_prev)
                                {
                                    Some(idx) => { index = idx; },
                                    _ => { index = usize::MAX; },
                                }
                                if index != usize::MAX { ddlist_w.remove(index); }
                            },
                            'S' =>
                            {
                                let (_, settings_local) = text.split_at(TAG_SETTINGS.len());
                                textbox_settings.clear();
                                textbox_settings.append(settings_local);

                            },
                            'E' => 
                            {
                                let (_, err_msg) = text.split_at(TAG_ERR.len());
                                nwg::modal_error_message(
                                    &window_main.clone().handle,
                                    "ERROR",
                                    err_msg
                                );
                            },
                            _ =>
                            { field_output.set_text(&text); },
                        }
                    },
                    _ => 
                    {
                    },
                }      
            },
        }
    });                                               
    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}