use std::{
    env,
    error::Error,
    fs, iter,
    process::{exit, Command},
    str,
};

type AnyError = Box<dyn Error>;

struct JsonScanner<'src> {
    cs: iter::Peekable<str::Chars<'src>>,
    i: usize,
    json: &'src str,
}

#[derive(PartialEq, Eq, Debug)]
enum JsonEvent<'src> {
    EnterObj,         // {
    ExitObj,          // }
    EnterList,        // [
    ExitList,         // ]
    Entry(&'src str), // <str>:
    Str(&'src str),   // <str>[,]
}

impl<'src> JsonScanner<'src> {
    fn new(json: &'src str) -> Self {
        Self {
            i: 0,
            cs: json.chars().peekable(),
            json: json,
        }
    }
}

impl<'src> Iterator for JsonScanner<'src> {
    type Item = JsonEvent<'src>;

    fn next(&mut self) -> Option<Self::Item> {
        use JsonEvent::*;

        while let Some(c) = self.cs.next() {
            self.i += c.len_utf8();
            if c == '{' {
                return Some(EnterObj);
            } else if c == '}' {
                return Some(ExitObj);
            } else if c == '[' {
                return Some(EnterList);
            } else if c == ']' {
                return Some(ExitList);
            } else if c == '"' {
                let start = self.i; // first char after quotes
                while let Some(c) = self.cs.next() {
                    match c {
                        '\\' => {
                            self.i += c.len_utf8();
                            let c = self.cs.next().unwrap();
                            self.i += c.len_utf8();
                        }
                        '"' => {
                            let jstr = &self.json[start..self.i];
                            self.i += c.len_utf8();
                            match self.cs.peek() {
                                Some(':') => return Some(Entry(jstr)),
                                _ => return Some(Str(jstr)),
                            }
                        }
                        _ => self.i += c.len_utf8(),
                    }
                }

                return None; // unrechable (assuming given json is correct)
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
        return Err(String::from_utf8(output.stderr)?.into());
    }

    let metadata = String::from_utf8(output.stdout)?;
    Ok(metadata)
}

fn open_metadata_file(fpath: String) -> Result<String, AnyError> {
    let metadata = fs::read_to_string(fpath)?;
    Ok(metadata)
}

fn get_dependencies(metadata: &str) -> Vec<&str> {
    use JsonEvent::*;
    let scanner = JsonScanner::new(metadata);

    let mut dependencies = vec![];

    let mut tp_lvl = false; // top level
    let mut pkg = false; // package
    let mut pkg_lst = false; // package list
    let mut pkg_lst_itm = false; // package list item
    let mut mnfst = false; // manifest
    let mut dp = 0; // depth

    // {                            (dp == 1 && EnterObj)
    //     "package":               (dp == 1 && tp_lvl && Entry("package"))
    //     [                        (dp == 2 && tp_lvl && pkg && EnterList)
    //         {                    (dp == 3 && tp_lvl && pkg && pkg_lst && EnterObj)
    //             ...,
    //             "manifest_path": (dp == 3 && tp_lvl && pkg && pkg_lst && pkg_lst_itm && Entry("manifest_path"))
    //             Str(...),        (dp == 3 && tp_lvl && pkg && pkg_lst && pkg_lst_itm && mnfst && Str(<path>))
    //             ...
    //         },                   (dp == 2 && tp_lvl && pkg && pkg_lst && pkg_lst_itm, ExitObj)
    //         ...
    //     ],                       (dp == 1 && tp_lvl && pkg && pkg_lst && ExitList) -> break
    //     ...
    // }
    for event in scanner {
        if event == EnterObj || event == EnterList {
            dp += 1;
        } else if event == ExitObj || event == ExitList {
            dp -= 1;
        }

        match (dp, tp_lvl, pkg, pkg_lst, pkg_lst_itm, mnfst, event) {
            (1, false, .., EnterObj) => tp_lvl = true,
            (1, true, false, .., Entry("packages")) => pkg = true,
            (2, true, true, false, .., EnterList) => pkg_lst = true,
            (3, true, true, true, false, .., EnterObj) => pkg_lst_itm = true,
            (3, true, true, true, true, .., Entry("manifest_path")) => mnfst = true,
            (3, true, true, true, true, true, event) => {
                if let Str(path) = event {
                    dependencies.push(path);
                }
                mnfst = false;
            }
            (2, true, true, true, .., ExitObj) => pkg_lst_itm = false,
            (1, true, true, .., ExitList) => break,
            _ => (),
        }
    }

    return dependencies;
}

fn real_main() -> Result<i32, AnyError> {
    let mut args = env::args();
    args.next();
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
