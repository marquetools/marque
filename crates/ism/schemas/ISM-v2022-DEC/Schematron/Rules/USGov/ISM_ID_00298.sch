<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00298" is-a="AttributeContributesToRollupWithException">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00298][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
    document specifies attribute @ism:atomicEnergyMarkings with a value containing [TFNI] and no elements
    meeting ISM_CONTRIBUTES having the attribute @ism:atomicEnergyMarkings containing [RD] or [FRD],
    then the ISM_RESOURCE_ELEMENT must specify attribute @ism:atomicEnergyMarkings with a value containing [TFNI].
    
    Human Readable: USA documents having Transclassified Foreign Nuclear Information (TFNI)
    and not having Restricted Data (RD) or Formerly Restricted Data (FRD) must have TFNI at the resource level.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    This rule uses an abstract pattern to consolidate logic. If the document
    is an ISM_USGOV_RESOURCE and an element meeting ISM_CONTRIBUTES
    specifies $attrLocalName with a value containing the token $value, then 
    this rule ensures that the ISM_RESOURCE_ELEMENT specifies the attribute
    $attrLocalName with a value containing the token $value.
  </sch:p>
  <sch:param name="attrLocalName" value="atomicEnergyMarkings"/>
  <sch:param name="exceptAttrLocalName" value="atomicEnergyMarkings"/>
  <sch:param name="value" value="TFNI"/>
  <sch:param name="exceptValueList" value="('RD', 'FRD')"/>
  <sch:param name="errorMessage" value="'[ISM-ID-00298][Error] USA documents having Transclassified Foreign Nuclear Information (TFNI)     and not having Restricted Data (RD) or Formerly Restricted Data (FRD) must have TFNI at the resource level.'"/>
</sch:pattern>