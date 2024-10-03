use std::{
    env,
    error::Error,
    fs,
    iter,
    process::{Command, exit},
    str,
};

type AnyError = Box<dyn Error>;


struct JsonScanner<'src> {
    // i: usize,
    cs: iter::Peekable<str::Chars<'src>>,
}


#[derive(PartialEq, Eq, Debug)]
enum JsonEvent {
    EnterObj,       // {
    ExitObj,        // }
    EnterList,      // [
    ExitList,       // ]
    Entry(String),  // <str>:
    Str(String),    // <str>[,]
}


impl<'src> JsonScanner<'src> {
    fn new(source: &'src str) -> Self {
        Self {
            // i: 0,
            cs: source.chars().peekable()
        }
    }
}

impl<'src> Iterator for JsonScanner<'src> {
    type Item = JsonEvent;

    fn next(&mut self) -> Option<Self::Item> {
        use JsonEvent::*;

        while let Some(c) = self.cs.next() {
            if c == '{' {
                return Some(EnterObj)
            } else if c == '}' {
                return Some(ExitObj)
            } else if c == '[' {
                return Some(EnterList)
            } else if c == ']' {
                return Some(ExitList)
            } else if c == '"' {
                let mut str_buf = String::new();
                while let Some(c) = self.cs.next() {
                    match c {
                        '\\' => {
                            str_buf.push(c);
                            str_buf.push(self.cs.next().unwrap());
                        },
                        '"' => match self.cs.peek() {
                            Some(':') => return Some(Entry(str_buf)),
                            _ => return Some(Str(str_buf)),
                        }
                        _ => str_buf.push(c),
                    }
                }

                return None // unrechable (assuming given json is correct)
            }
        }

        None
    }
}


fn use_cargo_metadata() -> Result<String, AnyError> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()?;

    if !output.status.success() {
        return Err(String::from_utf8(output.stderr)?.into())
    }

    let metadata = String::from_utf8(output.stdout)?;
    Ok(metadata)
}


fn open_metadata_file(fpath: String) -> Result<String, AnyError> {
    let metadata = fs::read_to_string(fpath)?;
    Ok(metadata)
}


fn get_dependencies(metadata: &str) -> Vec<String> {
    use JsonEvent::*;
    let scanner = JsonScanner::new(metadata);

    let mut dependencies = vec![];

    let mut tp_lvl = false;         // top-level
    let mut pkg = false;            // package
    let mut pkg_lst = false;        // package-list
    let mut pkg_lst_itm = false;    // package-list-item
    let mut mnfst = false;          // manifest
    let mut dp = 0;                 // depth

    // {                            (dp == 0 && EnterObj)
    //     "package":               (dp == 1 && tp_lvl && Entry("package"))
    //     [                        (dp == 1 && tp_lvl && pkg && EnterList)
    //         {                    (dp == 2 && tp_lvl && pkg && pkg_lst && EnterObj)
    //             ...,
    //             "manifest_path": (dp == 3 && tp_lvl && pkg && pkg_lst && pkg_lst_itm && Entry("manifest_path"))
    //             Str(...),        (dp == 3 && tp_lvl && pkg && pkg_lst && pkg_lst_itm && mnfst && Str(<path>))
    //             ...
    //             },               (dp == 3 && tp_lvl && pkg && pkg_lst && pkg_lst_itm, ExitObj)
    //         ...
    //         ],                   (dp == 2 && tp_lvl && pkg && pkg_lst && ExitList) -> break
    //     ...
    //     }
    for event in scanner {
        println!("(dp={dp}, tp_lvl={tp_lvl}, pkg={pkg}, pkg_lst={pkg_lst}, pkg_lst_itm={pkg_lst_itm}, mnfst={mnfst}, event={event:?})");
        match (dp, tp_lvl, pkg, pkg_lst, pkg_lst_itm, mnfst, event) {
            (0, false, .., EnterObj) => { dp += 1; tp_lvl = true },

            (1, true, false, .., Entry(e)) if e == "packages" => pkg = true,
            (1, true, true, false, .., EnterList) => { dp += 1; pkg_lst = true }

            (2, true, true, true, false, .., EnterObj) => { dp += 1; pkg_lst_itm = true }

            (3, true, true, true, true, .., Entry(e)) if e == "manifest_path" => { mnfst = true }
            (3, true, true, true, true, true, Str(path)) => { dependencies.push(path); mnfst = false }
            (3, true, true, true, true, true, _) =>  mnfst = false,
            (3, true, true, true, .., ExitObj) => { dp -= 1; pkg_lst_itm = false }

            (2, true, true, .., ExitList) => {
                dp -= 1;
                pkg=false;
                pkg_lst=false;
                println!("early break");
                println!("(dp={dp}, tp_lvl={tp_lvl}, pkg={pkg}, pkg_lst={pkg_lst}, pkg_lst_itm={pkg_lst_itm}, mnfst={mnfst}, event=ExitList)");
                break
            },

            (.., EnterObj|EnterList) => dp += 1,
            (.., ExitObj|ExitList) => dp -= 1,
            _ => (),
        }
    }

    return dependencies
}


fn real_main() -> Result<i32, AnyError> {
    let mut args = env::args();
    args.next(); // ingnoring program name
    let sample_file = args.next();
    let metadata = sample_file
        .map(open_metadata_file)
        .unwrap_or_else(use_cargo_metadata)?;

    let dependencies = get_dependencies(&metadata);
    for dep in dependencies.into_iter() {
        println!("{dep}");
    }
    Ok(0)
}


fn main() {
    let code = real_main().unwrap_or_else(|err| {
        eprintln!("{err}");
        1
    });

    exit(code);
}
