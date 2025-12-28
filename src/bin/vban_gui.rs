use gtk::{self, Align, Expression, StringList};
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, Orientation};
use gtk::gdk::Display;
use glib::clone;
use pipewire::keys::{APP_NAME, NODE_DESCRIPTION, NODE_NAME, NODE_NICK};

use std::net::IpAddr;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use pipewire::{context::Context, keys::{MEDIA_CLASS}, main_loop::MainLoop};

use rvban::{VBanCodec, VBanSampleRates};

const SAMPLE_RATES : [VBanSampleRates; 7] = 
    [VBanSampleRates::SampleRate6000Hz,
    VBanSampleRates::SampleRate12000Hz,
    VBanSampleRates::SampleRate24000Hz,
    VBanSampleRates::SampleRate44100Hz,
    VBanSampleRates::SampleRate48000Hz,
    VBanSampleRates::SampleRate96000Hz,
    VBanSampleRates::SampleRate192000Hz,];
    // VBanSampleRates::SampleRate384000Hz,
    // VBanSampleRates::SampleRate8000Hz,
    // VBanSampleRates::SampleRate16000Hz,
    // VBanSampleRates::SampleRate32000Hz,
    // VBanSampleRates::SampleRate64000Hz,
    // VBanSampleRates::SampleRate128000Hz,
    // VBanSampleRates::SampleRate256000Hz,
    // VBanSampleRates::SampleRate512000Hz,
    // VBanSampleRates::SampleRate11025Hz,
    // VBanSampleRates::SampleRate22050Hz,
    // VBanSampleRates::SampleRate88200Hz,
    // VBanSampleRates::SampleRate176400Hz,
    // VBanSampleRates::SampleRate352800Hz,
    // VBanSampleRates::SampleRate705600Hz,];

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("com.lennard.vban_gui")
        .build();

    app.connect_activate(build_ui);

    app.run()

}

fn get_pw_app_names(name_list : &Arc<Mutex<Vec<String>>>){
    let list = Arc::clone(&name_list);
    
    thread::spawn(move ||{

        let mainloop = MainLoop::new(None).unwrap();
        let context = Context::new(&mainloop)
        .unwrap();
        let core = context.connect(None).unwrap();
        let registry = core.get_registry().unwrap();

        let _listener = registry
            .add_listener_local()
            .global(move |global| {
                    if global.type_.to_str() == "PipeWire:Interface:Node" {
                        let props = match global.props {
                            None => return,
                            Some(p) => p
                        };

                        // println!("{}", global.type_.to_str());

                        let class = match props.get(&MEDIA_CLASS){
                            None => return,
                            Some(class) => class
                        };

                        // println!("\t {}", class);

                        if ! class.contains("Audio"){
                            return;
                        }

                        let mut name = props.get(&APP_NAME);
                        if name.is_none() {
                            name = props.get(&NODE_NICK);
                        }
                        if name.is_none() {
                            name = props.get(&NODE_NAME);
                        }
                        if name.is_none() {
                            name = props.get(&NODE_DESCRIPTION);
                        }
                        if name.is_none(){
                            name = Some("Nameless app");
                        }

                        println!("\t Name: {}", name.unwrap());
                        // list.push(name.unwrap().clone());
                        list.lock().unwrap().push(name.unwrap().to_string());
                    }
                })
            .register();

        mainloop.run();
    });

}


fn load_css() {
     // Provide css

    let css = r#"
    /* base styling for our toggle button when active */
    .toggle-active {
        color: red;
        font-weight: bold;
    }

    /* slightly different when not active */
    .toggle-inactive {
        color: skyblue;
        font-weight: bold;
    }

    /* make headline larger and centered */
    .headline {
        font-size: 20px;
        font-weight: bold;
    }

    .faulty-input {
        background-color: #7a1c1c9f;
    }

    .good-input {
        background-color: #307223a1;
    }

    "#;

    let provider = gtk::CssProvider::new();
    provider.load_from_string(css);
    gtk::style_context_add_provider_for_display(
        &Display::default().expect("No display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}


fn build_ui(app: &Application) {

    let peer : Rc<Cell<(IpAddr, u16)>> = Rc::new(Cell::new((IpAddr::V4("192.168.178.75".parse().unwrap()), 6980)));
    let local_addr = (IpAddr::V4("0.0.0.0".parse().unwrap()), 0); // default VBAN port
    let stream_name= String::from("Stream1");
    let numch = 2;
    let sample_rate= Rc::new(Cell::new(VBanSampleRates::SampleRate48000Hz));
    let format= rvban::VBanBitResolution::VbanBitfmt16Int;
    let source_name = Rc::new(RefCell::new(String::from("spotify")));
    let encoder  = Rc::new(Cell::new(VBanCodec::VbanCodecPcm.into()));
    let handle = Rc::new(RefCell::new(Option::<std::thread::JoinHandle<()>>::None));

    let app_names = Arc::new(Mutex::new(Vec::new()));
    
    load_css();

    get_pw_app_names(&app_names);

    // allow some time to register all applications
    sleep(Duration::from_millis(200));

    let vbox = gtk::Box::builder()
    .orientation(gtk::Orientation::Vertical)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .spacing(12)
        .build();

    // 1) Headline 
    let headline = gtk::Label::builder()
        .label("VBAN Sender")
        .halign(gtk::Align::Center)
        .css_classes(vec!["headline"])
        .margin_bottom(6)
        .build();

    vbox.append(&headline);

    // 2) Labeled entry
    let entry_row = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::Start)
        .build();

    let entry_label = gtk::Label::builder()
        .label("Receiver IP:")
        .halign(gtk::Align::Start)
        .build();

    let entry = gtk::Entry::builder()
        .hexpand(true)  // expand to fill available space
        .build();

    let buffer = entry.buffer();
    buffer.set_text(peer.get().0.to_string());


    entry.connect_changed(clone!(
        #[weak]
        peer,
        move |e| {
        let text = e.text().to_string();
        println!("New text: {text}");
        // check if the text content has the form of an ip address
        if text.parse::<std::net::IpAddr>().is_ok(){
            eprintln!("Is valid IP address.");
            e.remove_css_class("faulty-input");
            e.add_css_class("good-input");

            let ip: std::net::IpAddr = text.parse().unwrap();
            peer.set((ip, 6980)); // default VBAN port
        } else{
            e.remove_css_class("good-input");
            e.add_css_class("faulty-input");
        }
    }));

    entry_row.append(&entry_label);
    entry_row.append(&entry);
    vbox.append(&entry_row);

    // 3) Labeled dropdown
    let combo_row = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Fill)
        .build();

    let combo_label = gtk::Label::builder()
        .label("Sample Rate:")
        .build();

    let mut rates = Vec::new();
    for rate in SAMPLE_RATES.iter() {
        rates.push(format!("{}", rate) );
    }

    // let combo = gtk::DropDown::from_strings(&rates.iter().map(AsRef::as_ref).collect::<Vec<&str>>());
    // combo.set_model(Some(&model));
    
    let model = StringList::new(&rates.iter().map(AsRef::as_ref).collect::<Vec<&str>>());
    let combo = gtk::DropDown::new(Some(model), None::<Expression>);

    combo.set_selected(4); // set to 48 kHz

    combo.connect_selected_item_notify(clone!(
        #[strong] sample_rate,
        move |c| {
        let num = c.selected();
        eprintln!("Dropdown selected item changed: {}", SAMPLE_RATES[num as usize]);
        sample_rate.set(SAMPLE_RATES[num as usize]);
    }));

    combo_row.append(&combo_label);
    combo_row.append(&combo);
    vbox.append(&combo_row);


    // 4) Labeled radio buttons
    let radios_label = gtk::Label::builder()
        .label("Codec:")
        .margin_top(6)
        .margin_bottom(6)
        .halign(gtk::Align::Start)
        .build();

    vbox.append(&radios_label);

    // vertival box for radio list
    let radio_list = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        // .halign(gtk::Align::Start)
        .build();

    let r1 = gtk::CheckButton::with_label("PCM");
    r1.connect_toggled(clone!(
        #[strong] encoder, 
        move |r|{
        if r.is_active() {
            println!("Selected PCM");
            encoder.set(VBanCodec::VbanCodecPcm.into());
        }
    }));
    let r2 = gtk::CheckButton::with_label("Opus");
    r2.connect_toggled(clone!(
        #[strong] encoder,
        move |r|{
        if r.is_active() {
            encoder.set(VBanCodec::VbanCodecOpus(None).into());
            println!("Selected Opus");
        }
    }));

    r2.set_group(Some(&r1));
    r2.set_active(true);
    radio_list.append(&r1);
    radio_list.append(&r2);
    
    vbox.append(&radio_list);

    // 5) Dropdown for application names
    let app_names_row = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Fill)
        .build();

    let app_names_label = gtk::Label::builder()
        .label("Quelle:")
        .build();

    // let app_names_dd = gtk::DropDown::from_strings(&app_names.lock().unwrap().iter().map(AsRef::as_ref).collect::<Vec<&str>>());

    let app_names_model = StringList::new(&app_names.lock().unwrap().iter().map(AsRef::as_ref).collect::<Vec<&str>>());
    let app_names_dd = gtk::DropDown::new(Some(app_names_model), None::<Expression>);


    app_names_dd.connect_selected_item_notify(clone!(
        #[strong] source_name,
        move |dd_menu| {
        let num = dd_menu.selected();
        *source_name.borrow_mut() = app_names.lock().unwrap()[num as usize].clone();
        eprintln!("Selected audio source: {}", num);
    }));

    app_names_row.append(&app_names_label);
    app_names_row.append(&app_names_dd);
    vbox.append(&app_names_row);

    // 6) Toggle Button
    let toggle = gtk::ToggleButton::builder()
        .label("Activate")
        .halign(Align::Center)
        .margin_top(12)
        .build();

    toggle.add_css_class("toggle-inactive");

    toggle.connect_toggled(clone!(
        #[strong] encoder,
        #[strong] handle,
        #[strong] source_name,
        move |toggle| {

            if toggle.is_active() {
                println!("Activated");

                let mut vbs = match rvban::vban_sender_pw::VbanSender::create(peer.get(), local_addr, stream_name.clone(), numch, sample_rate.get(), format, source_name.borrow().to_string(), encoder.get()) {
                    None => {
                            println!("Error: Could not create VBAN Sender");
                            return;
                        }
                    Some(sender) => sender
                };

                let new_handle = std::thread::spawn(move || {
                    loop {
                        vbs.handle();
                    }
                });

                handle.borrow_mut().replace(new_handle);

                entry.set_sensitive(false);
                combo.set_sensitive(false);
                r1.set_sensitive(false);
                app_names_dd.set_sensitive(false);


                toggle.set_label("Quit");
                toggle.remove_css_class("toggle-inactive");
                toggle.add_css_class("toggle-active");
            } else {
                toggle.set_label("Activate");
                toggle.remove_css_class("toggle-active");
                toggle.add_css_class("toggle-inactive");
                println!("Deactivated");

                if handle.borrow().is_some(){
                    exit(0);
                }
            }
        }
    ));

    vbox.append(&toggle);

    // Create the main window.
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(500)
        .default_height(300)
        .title("VBAN Sender")
        .child(&vbox)
        .build();

    
    // Show the window.
    window.present();
}