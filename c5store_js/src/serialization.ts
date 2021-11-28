import yaml from "js-yaml";

export interface C5ValueDeserializer {
  deserialize(data: any): any;
}

export class C5JSONValueDeserializer implements C5ValueDeserializer {

  deserialize(data: any): any {
    
    return JSON.parse(data);
  }
}

export class C5YAMLValueDeserializer implements C5ValueDeserializer {

  deserialize(data: any): any {
    
    return yaml.load(data);
  }
}