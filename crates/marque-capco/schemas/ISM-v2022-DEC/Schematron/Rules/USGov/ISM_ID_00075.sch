<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00075" is-a="AttributeContributesToRollupWithException">
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
    [ISM-ID-00075][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the 
    document having the attribute @ism:atomicEnergyMarkings containing [FRD] and no elements
    meeting ISM_CONTRIBUTES having the attribute @ism:atomicEnergyMarkings containing [RD], then the 
    ISM_RESOURCE_ELEMENT must have @ism:atomicEnergyMarkings containing [FRD].
    
    Human Readable: USA documents having Formerly Restricted Data (FRD) and not having Restricted Data (RD) 
    must have FRD at the resource level.
  </sch:p>
  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    This rule uses an abstract pattern to consolidate logic. For details on the code
    description, see the abstract pattern.
  </sch:p>
  <sch:param name="attrLocalName" value="atomicEnergyMarkings"/>
  <sch:param name="exceptAttrLocalName" value="atomicEnergyMarkings"/>
  <sch:param name="value" value="FRD"/>
  <sch:param name="exceptValueList" value="('RD')"/>
  <sch:param name="errorMessage" value="'[ISM-ID-00075][Error] USA documents having Formerly Restricted Data (FRD) and not having Restricted Data (RD) must have FRD at the resource level.'"/>
</sch:pattern>