//! Shared artwork variants and pose assignments. The compiler feeds the
//! existing layered renderer, so authoring and legacy packs share one rig.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::placement::Placement;

#[derive(Debug, Clone)]
pub struct Composition {
    pub variants: BTreeMap<String, Value>,
    pub poses: BTreeMap<String, Value>,
}

impl Composition {
    pub(in crate::mascot) fn from_poses(poses: &[super::pack::Pose]) -> Self {
        let mut composition = Self {
            variants: BTreeMap::new(),
            poses: BTreeMap::new(),
        };
        for pose in poses {
            let mut slots = serde_json::Map::new();
            let body = json!({"kind":if pose.parts.is_empty() && pose.eyes.is_empty() {"complete"}else{"body"},"image":pose.body});
            let id = composition.intern_variant(&format!("body.{}", pose.name), body);
            slots.insert("body".into(), json!(id));
            let encoded = pose.to_json();
            let mut names = Vec::new();
            let mut counts = BTreeMap::<String, usize>::new();
            for (n, part) in pose.parts.iter().enumerate() {
                let kind = part.kind.name();
                let count = counts.entry(kind.into()).or_default();
                *count += 1;
                let slot = if *count == 1 {
                    kind.to_owned()
                } else {
                    format!("{kind}-{}", *count)
                };
                let mut definition = encoded["parts"][n].clone();
                definition.as_object_mut().unwrap().remove("type");
                definition.as_object_mut().unwrap().remove("parent");
                definition["kind"] = json!(kind);
                let scoped = matches!(
                    part.kind,
                    super::motion::PartType::LeftArm
                        | super::motion::PartType::RightArm
                        | super::motion::PartType::HoldingArm
                        | super::motion::PartType::Grip
                );
                let id = if scoped {
                    let id = format!("{slot}.{}", pose.name);
                    composition.variants.insert(id.clone(), definition);
                    id
                } else {
                    composition.intern_variant(&format!("{slot}.front"), definition)
                };
                slots.insert(slot.clone(), json!(id));
                names.push(slot);
            }
            for (n, part) in pose.parts.iter().enumerate() {
                if let Some(parent) = part.parent {
                    let id = slots[&names[n]].clone();
                    slots[&names[n]] = json!({"variant":id,"parent":names[parent]});
                }
            }
            if !pose.eyes.is_empty() {
                let id = composition
                    .intern_variant("eyes.front", json!({"kind":"eyes","eyes":encoded["eyes"]}));
                slots.insert("eyes".into(), json!(id));
            }
            composition.poses.insert(
                pose.name.clone(),
                json!({"ground_anchor":[pose.ground.0,pose.ground.1],"slots":slots}),
            );
        }
        composition
    }

    /// Keep authored sleeping and carrying pictures as explicit complete poses.
    pub(in crate::mascot) fn add_complete_reactions(&mut self, reactions: &Value) {
        for (name, art, mouth) in [
            (
                "doze",
                &reactions["sleep"]["visual"],
                &reactions["companionship"]["mouth"],
            ),
            (
                "curled",
                &reactions["sleep"]["sleep_poses"][0]["art"],
                &reactions["sleep"]["sleep_poses"][0]["mouth"],
            ),
            ("held", &reactions["pickup"]["held_pose"], &Value::Null),
        ] {
            if self.poses.contains_key(name) {
                continue;
            }
            if let Some(image) = art["image"].as_str() {
                let id = self.intern_variant(
                    &format!("complete.{name}"),
                    json!({"kind":"complete","image":image}),
                );
                self.poses.insert(name.into(),json!({"ground_anchor":[0.5,0.9],"slots":{"body":id},"reaction_place":art["place"],"mouth":mouth}));
            }
        }
    }

    /// Reactions reference pose identity through its body asset; the reaction
    /// loader renders the complete resolved recipe using the ordinary rig.
    pub(in crate::mascot) fn resolve_reactions(&self, reactions: &mut Value) -> Result<(), String> {
        for name in ["doze", "curled", "held"] {
            let Some(recipe) = self.poses.get(name) else {
                continue;
            };
            let (body, _) = self.slot(recipe, "body")?;
            let art = json!({"pose":name,"image":body["image"],"place":recipe["reaction_place"].as_array().cloned().unwrap_or_else(||vec![json!(0.0),json!(0.0),json!(1.0)])});
            if reactions.is_null() {
                *reactions = json!({});
            }
            let event = if name == "held" { "pickup" } else { "sleep" };
            if reactions[event].is_null() {
                reactions[event] = json!({});
            }
            match name {
                "doze" => reactions["sleep"]["visual"] = art,
                "held" => reactions["pickup"]["held_pose"] = art,
                _ => {
                    if reactions["sleep"]["sleep_poses"].is_null() {
                        reactions["sleep"]["sleep_poses"] = json!([]);
                    }
                    let entry = json!({"art":art,"mouth":recipe["mouth"].as_array().cloned().unwrap_or_else(||vec![json!(0.4),json!(0.6)])});
                    let entries = reactions["sleep"]["sleep_poses"]
                        .as_array_mut()
                        .ok_or("sleep poses must be a list")?;
                    if entries.is_empty() {
                        entries.push(entry);
                    } else {
                        entries[0] = entry;
                    }
                }
            }
        }
        Ok(())
    }

    pub(in crate::mascot) fn group_static_eyes(&mut self, left: &str, right: &str) {
        let ids: Vec<_> = self
            .variants
            .iter()
            .filter(|(_, v)| v["image"] == left || v["image"] == right)
            .map(|(id, _)| id.clone())
            .collect();
        if ids.len() != 2 {
            return;
        }
        let id = self.intern_variant(
            "eyes.front",
            json!({"kind":"eyes","eyes":[{"image":left},{"image":right}]}),
        );
        for pose in self.poses.values_mut() {
            if let Some(slots) = pose["slots"].as_object_mut() {
                let count = slots
                    .values()
                    .filter(|a| {
                        Self::variant_id(a).is_some_and(|id| ids.iter().any(|other| other == id))
                    })
                    .count();
                if count == 2 {
                    slots.retain(|_, a| {
                        !Self::variant_id(a).is_some_and(|id| ids.iter().any(|other| other == id))
                    });
                    slots.insert("eyes".into(), json!(id));
                }
            }
        }
        for id in ids {
            if self.uses(&id).is_empty() {
                self.variants.remove(&id);
            }
        }
    }

    fn intern_variant(&mut self, suggested: &str, value: Value) -> String {
        if let Some((id, _)) = self.variants.iter().find(|(_, v)| **v == value) {
            return id.clone();
        }
        let mut id = suggested.to_owned();
        let mut number = 1;
        while self.variants.contains_key(&id) {
            id = format!("{suggested}-{number}");
            number += 1;
        }
        self.variants.insert(id.clone(), value);
        id
    }
    pub fn parse(value: &Value) -> Result<Option<Self>, String> {
        if value.is_null() {
            return Ok(None);
        }
        if value.get("version").is_some_and(|v| v.as_u64() != Some(1)) {
            return Err("unsupported composition version".into());
        }
        let read = |key: &str| -> Result<BTreeMap<String, Value>, String> {
            let map = value[key]
                .as_object()
                .filter(|m| !m.is_empty())
                .ok_or_else(|| format!("composition needs {key}"))?;
            if map.len() > 64 {
                return Err(format!("too many composition {key}"));
            }
            Ok(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        };
        let composition = Self {
            variants: read("variants")?,
            poses: read("poses")?,
        };
        for (id, variant) in &composition.variants {
            if id.is_empty()
                || id.len() > 80
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
            {
                return Err(
                    "variant names use letters, numbers, dots, dashes and underscores".into(),
                );
            }
            let kind = variant["kind"]
                .as_str()
                .ok_or("each variant needs a kind")?;
            if !matches!(kind, "body" | "complete" | "eyes")
                && super::motion::PartType::parse(kind).is_none()
            {
                return Err(format!("unknown variant kind {kind}"));
            }
            if kind == "eyes" && variant["eyes"].as_array().is_none_or(|a| a.len() != 2) {
                return Err("an eye set needs exactly one left/right pair".into());
            }
            if kind != "eyes" && variant["image"].as_str().is_none() {
                return Err(format!("variant {id} needs an image"));
            }
        }
        Ok(Some(composition))
    }

    pub fn to_json(&self) -> Value {
        json!({"version":1,"variants":self.variants,"poses":self.poses})
    }

    pub fn image_fields(variant: &Value) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        if let Some(image) = variant["image"].as_str() {
            fields.push(("/image".into(), image.into()));
        }
        for expression in ["blink", "pressed", "dragging", "petted"] {
            let pointer = format!("/expressions/{expression}");
            if let Some(image) = variant.pointer(&pointer).and_then(Value::as_str) {
                fields.push((pointer, image.into()));
            }
        }
        if let Some(eyes) = variant["eyes"].as_array() {
            for (n, eye) in eyes.iter().enumerate() {
                for field in ["image", "white", "pupil", "mask", "blink"] {
                    if let Some(image) = eye[field].as_str() {
                        fields.push((format!("/eyes/{n}/{field}"), image.into()));
                    }
                }
            }
        }
        fields
    }

    pub fn files(&self) -> Vec<&str> {
        let mut files = BTreeSet::new();
        for variant in self.variants.values() {
            if let Some(sources) = variant["sources"].as_array() {
                files.extend(sources.iter().filter_map(Value::as_str));
            }
            if let Some(image) = variant["image"].as_str() {
                files.insert(image);
            }
            if let Some(expressions) = variant["expressions"].as_object() {
                files.extend(expressions.values().filter_map(Value::as_str));
            }
            if let Some(eyes) = variant["eyes"].as_array() {
                for eye in eyes {
                    for field in ["image", "white", "pupil", "mask", "blink"] {
                        if let Some(image) = eye[field].as_str() {
                            files.insert(image);
                        }
                    }
                }
            }
        }
        files.into_iter().collect()
    }

    pub fn variant_id(assignment: &Value) -> Option<&str> {
        assignment
            .as_str()
            .or_else(|| assignment["variant"].as_str())
    }

    pub fn uses(&self, id: &str) -> Vec<String> {
        self.poses
            .iter()
            .filter(|(_, pose)| {
                pose["slots"]
                    .as_object()
                    .is_some_and(|slots| slots.values().any(|v| Self::variant_id(v) == Some(id)))
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    fn slot<'a>(&'a self, pose: &'a Value, slot: &str) -> Result<(&'a Value, &'a Value), String> {
        let assignment = pose["slots"]
            .get(slot)
            .ok_or_else(|| format!("missing pose slot {slot}"))?;
        let id = Self::variant_id(assignment).ok_or("pose assignment needs a variant")?;
        let variant = self
            .variants
            .get(id)
            .ok_or_else(|| format!("missing variant {id}"))?;
        Ok((variant, assignment))
    }

    fn placement(
        &self,
        pose: &Value,
        slot: &str,
        canvas: (u32, u32),
        visiting: &mut BTreeSet<String>,
    ) -> Result<Placement, String> {
        if !visiting.insert(slot.into()) {
            return Err("pose attachments contain a cycle".into());
        }
        let (_, assignment) = self.slot(pose, slot)?;
        let placement = match assignment["placement"].as_array() {
            None => Placement::default(),
            Some(values) if values.len() == 4 => Placement {
                x: values[0].as_f64().ok_or("invalid pose offset")? as f32 * canvas.0 as f32,
                y: values[1].as_f64().ok_or("invalid pose offset")? as f32 * canvas.1 as f32,
                scale: values[2].as_f64().ok_or("invalid pose scale")? as f32,
                angle: values[3].as_f64().ok_or("invalid pose angle")? as f32,
            },
            _ => return Err("pose placement needs x, y, scale and angle".into()),
        };
        if !placement.valid(canvas) {
            return Err("pose placement is outside the supported range".into());
        }
        let parent = assignment["parent"].as_str().or_else(|| {
            if slot == "eyes" {
                Some("head")
            } else if slot != "body" {
                Some("body")
            } else {
                None
            }
        });
        let result = if let Some(parent) = parent {
            let parent = self.placement(pose, parent, canvas, visiting)?;
            compose(parent, placement, canvas)
        } else {
            placement
        };
        visiting.remove(slot);
        Ok(result)
    }

    pub fn image_placement(
        &self,
        name: &str,
        image: &str,
        canvas: (u32, u32),
    ) -> Result<Placement, String> {
        let pose = self.poses.get(name).ok_or("missing pose")?;
        for (slot, assignment) in pose["slots"].as_object().ok_or("pose needs slots")? {
            let variant = &self.variants[Self::variant_id(assignment).ok_or("missing variant")?];
            if Self::image_fields(variant)
                .iter()
                .any(|(_, file)| file == image)
            {
                return self.placement(pose, slot, canvas, &mut BTreeSet::new());
            }
        }
        Ok(Placement::default())
    }

    pub fn compile(&self, canvas: (u32, u32)) -> Result<Value, String> {
        let mut compiled = serde_json::Map::new();
        for (name, recipe) in &self.poses {
            let slots = recipe["slots"].as_object().ok_or("each pose needs slots")?;
            let (body, _) = self.slot(recipe, "body")?;
            if !matches!(body["kind"].as_str(), Some("body" | "complete")) {
                return Err("body slot needs a body variant".into());
            }
            if body["kind"] == "complete" && slots.len() != 1 {
                return Err("a complete pose already contains its head, eyes and limbs".into());
            }
            let heads: Vec<_> = slots
                .values()
                .filter_map(Self::variant_id)
                .filter_map(|id| self.variants.get(id))
                .filter(|v| v["kind"] == "head")
                .collect();
            if heads.len() > 1 {
                return Err("each pose can have only one active head".into());
            }
            if slots.contains_key("eyes") && heads.iter().any(|head| head["eye_mode"] == "embedded")
            {
                return Err("this head already contains its eyes; remove the separate eye pair or choose a plain head".into());
            }
            let mut parts = Vec::new();
            let mut indexes = BTreeMap::new();
            let mut eyes = Value::Null;
            for (slot, assignment) in slots {
                let id = Self::variant_id(assignment).ok_or("pose assignment needs a variant")?;
                let variant = self
                    .variants
                    .get(id)
                    .ok_or_else(|| format!("missing variant {id}"))?;
                let kind = variant["kind"].as_str().ok_or("missing variant kind")?;
                let placement = self.placement(recipe, slot, canvas, &mut BTreeSet::new())?;
                match slot.as_str() {
                    "body" => {}
                    "eyes" => {
                        if assignment["parent"]
                            .as_str()
                            .is_some_and(|parent| parent != "head")
                        {
                            return Err("the eye pair must attach to its head".into());
                        }
                        if kind != "eyes" {
                            return Err("eyes slot needs an eye-pair variant".into());
                        }
                        eyes = variant["eyes"].clone();
                    }
                    _ => {
                        if super::motion::PartType::parse(kind).is_none() {
                            return Err(format!("{slot} needs a moving-part variant"));
                        }
                        let mut part = variant.clone();
                        part.as_object_mut().unwrap().remove("kind");
                        part.as_object_mut().unwrap().remove("eye_mode");
                        part.as_object_mut().unwrap().remove("sources");
                        part["type"] = json!(kind);
                        let pivot = assignment
                            .get("pivot")
                            .unwrap_or(&variant["pivot"])
                            .as_array()
                            .filter(|v| v.len() == 2)
                            .ok_or("part variant needs a pivot")?;
                        let point = (
                            pivot[0].as_f64().ok_or("invalid pivot")? as f32 * canvas.0 as f32,
                            pivot[1].as_f64().ok_or("invalid pivot")? as f32 * canvas.1 as f32,
                        );
                        let point = placement.map(point, canvas);
                        part["pivot"] =
                            json!([point.0 / canvas.0 as f32, point.1 / canvas.1 as f32]);
                        if let Some(layer) = assignment.get("layer") {
                            part["layer"] = layer.clone();
                        }
                        indexes.insert(slot.clone(), parts.len());
                        parts.push((slot.clone(), part));
                    }
                }
            }
            for (slot, part) in &mut parts {
                if let Some(parent) = slots[slot]["parent"]
                    .as_str()
                    .filter(|parent| *parent != "body")
                {
                    part["parent"] = json!(
                        indexes
                            .get(parent)
                            .ok_or_else(|| format!("missing parent slot {parent}"))?
                    );
                }
            }
            if slots.contains_key("eyes") && !parts.iter().any(|(_, p)| p["type"] == "head") {
                return Err("an eye pair needs a head assignment".into());
            }
            compiled.insert(name.clone(),json!({"body":body["image"],
                "ground_anchor":recipe["ground_anchor"],"parts":parts.into_iter().map(|(_,p)|p).collect::<Vec<_>>(),"eyes":eyes}));
        }
        Ok(Value::Object(compiled))
    }
}

fn compose(parent: Placement, child: Placement, canvas: (u32, u32)) -> Placement {
    let zero = parent.map(child.map((0.0, 0.0), canvas), canvas);
    let scale = parent.scale * child.scale;
    let angle = (parent.angle + child.angle + 180.0).rem_euclid(360.0) - 180.0;
    let (sin, cos) = angle.to_radians().sin_cos();
    let (cx, cy) = (canvas.0 as f32 / 2.0, canvas.1 as f32 / 2.0);
    Placement {
        x: zero.0 - scale * cx + scale * (cx * cos - cy * sin),
        y: zero.1 - scale * cy + scale * (cx * sin + cy * cos),
        scale,
        angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Composition {
        Composition::parse(&json!({"version":1,"variants":{
            "body.front":{"kind":"body","image":"body.png"},
            "head.front":{"kind":"head","image":"head.png","pivot":[0.5,0.5]},
            "eyes.front":{"kind":"eyes","eyes":[{"white":"left.png","pupil":"lp.png","mask":"lm.png"},{"white":"right.png","pupil":"rp.png","mask":"rm.png"}]},
            "arm.front":{"kind":"holding_arm","image":"arm.png","pivot":[0.5,0.6]},
            "phone.front":{"kind":"phone","image":"phone.png","pivot":[0.6,0.8]}
        },"poses":{"sit":{"ground_anchor":[0.5,0.9],"slots":{
            "body":"body.front","head":"head.front","eyes":"eyes.front",
            "holding_arm":{"variant":"arm.front","placement":[0.1,0.05,1,0]},
            "phone":{"variant":"phone.front","parent":"holding_arm"}
        }}}})).unwrap().unwrap()
    }
    #[test]
    fn static_pair_is_one_assignment_and_reaction_recipes_keep_distinct_identity() {
        let mut composition = fixture();
        composition.variants.get_mut("eyes.front").unwrap()["eyes"] =
            json!([{"image":"left.png"},{"image":"right.png"}]);
        let compiled = composition.compile((100, 100)).unwrap();
        assert_eq!(compiled["sit"]["eyes"][0]["image"], "left.png");
        assert!(composition.files().contains(&"right.png"));
        for name in ["doze", "held"] {
            composition
                .poses
                .insert(name.into(), composition.poses["sit"].clone());
        }
        let mut reactions = json!({});
        composition.resolve_reactions(&mut reactions).unwrap();
        assert_eq!(
            reactions["sleep"]["visual"]["image"],
            reactions["pickup"]["held_pose"]["image"]
        );
        assert_eq!(reactions["sleep"]["visual"]["pose"], "doze");
        assert_eq!(reactions["pickup"]["held_pose"]["pose"], "held");
    }
    #[test]
    fn shared_pair_and_attachment_roundtrip() {
        let mut composition = fixture();
        composition
            .poses
            .insert("stand".into(), composition.poses["sit"].clone());
        assert_eq!(composition.uses("eyes.front"), vec!["sit", "stand"]);
        let compiled = composition.compile((100, 100)).unwrap();
        let parts = compiled["sit"]["parts"].as_array().unwrap();
        let arm = parts
            .iter()
            .position(|p| p["type"] == "holding_arm")
            .unwrap();
        let phone = parts.iter().find(|p| p["type"] == "phone").unwrap();
        assert_eq!(phone["parent"], json!(arm));
        assert!((phone["pivot"][0].as_f64().unwrap() - 0.7).abs() < 0.0001);
        let place = composition
            .image_placement("sit", "phone.png", (100, 100))
            .unwrap();
        assert!((place.x - 10.0).abs() < 0.001);
        assert_eq!(
            Composition::parse(&composition.to_json())
                .unwrap()
                .unwrap()
                .compile((100, 100))
                .unwrap(),
            compiled
        );
    }
    #[test]
    fn rejects_missing_and_cyclic_attachments_and_duplicate_eyes() {
        let mut composition = fixture();
        composition.poses.get_mut("sit").unwrap()["slots"]["holding_arm"]["parent"] =
            json!("phone");
        assert!(
            composition
                .compile((100, 100))
                .unwrap_err()
                .contains("cycle")
        );
        composition.poses.get_mut("sit").unwrap()["slots"]["holding_arm"]["parent"] =
            json!("missing");
        assert!(
            composition
                .compile((100, 100))
                .unwrap_err()
                .contains("missing")
        );
        let mut json = fixture().to_json();
        json["variants"]["eyes.front"]["eyes"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(Composition::parse(&json).unwrap_err().contains("pair"));
    }
    #[test]
    fn embedded_face_cannot_stack_a_second_eye_pair() {
        let mut composition = fixture();
        composition.variants.get_mut("head.front").unwrap()["eye_mode"] = json!("embedded");
        assert!(
            composition
                .compile((100, 100))
                .unwrap_err()
                .contains("already contains its eyes")
        );
        composition.poses.get_mut("sit").unwrap()["slots"]
            .as_object_mut()
            .unwrap()
            .remove("eyes");
        composition.compile((100, 100)).unwrap();
    }

    #[test]
    fn complete_pose_excludes_separate_parts_and_sleep_reuses_closed_eyes() {
        let mut composition = fixture();
        composition.variants.get_mut("body.front").unwrap()["kind"] = json!("complete");
        assert!(
            composition
                .compile((100, 100))
                .unwrap_err()
                .contains("already contains")
        );
        let mut composition = fixture();
        composition.add_complete_reactions(&json!({"sleep":{"visual":{"image":"nap.png"},"sleep_poses":[{"art":{"image":"curled.png"},"mouth":[0.4,0.7]}]},"pickup":{"held_pose":{"image":"held.png"}}}));
        let mut reactions = json!({});
        composition.resolve_reactions(&mut reactions).unwrap();
        assert_eq!(reactions["sleep"]["visual"]["image"], "nap.png");
        assert_eq!(
            reactions["sleep"]["sleep_poses"][0]["art"]["image"],
            "curled.png"
        );
        assert_eq!(reactions["pickup"]["held_pose"]["image"], "held.png");
        composition.compile((100, 100)).unwrap();
    }
}
