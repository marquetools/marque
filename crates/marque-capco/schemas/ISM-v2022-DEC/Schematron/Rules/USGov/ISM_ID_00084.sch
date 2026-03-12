<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00084" is-a="AttributeContributesToRollupWithClassification">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00084][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
    have the attribute @ism:nonICmarkings containing [DS] then the ISM_RESOURCE_ELEMENT 
    must have @ism:nonICmarkings containing [DS]. 
    
    Human Readable: Unclassified USA documents having DS Data must have DS at the resource level.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    This rule uses an abstract pattern to consolidate logic. If the document
    is an ISM_USGOV_RESOURCE and an element meeting ISM_CONTRIBUTES
    specifies $attrLocalName with a value containing the token $value, then 
    this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    $attrLocalName with a value containing the token $value.
  </sch:p>
  <sch:param name="attrLocalName" value="nonICmarkings"/>
  <sch:param name="resourceElementClassification" value="U"/>
  <sch:param name="value" value="DS"/>
  <sch:param name="errorMessage" value="'[ISM-ID-00084][Error] Unclassified USA documents having DS Data must have DS at the resource level.'"/>
</sch:pattern>