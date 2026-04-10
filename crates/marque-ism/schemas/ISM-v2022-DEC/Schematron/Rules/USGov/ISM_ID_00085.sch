<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00085" is-a="AttributeContributesToRollupWithException">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00085][Error] If ISM_USGOV_RESOURCE and any element meeting ISM_CONTRIBUTES in the document 
        has the attribute @ism:nonICmarkings containing [XD] and does not have any element meeting ISM_CONTRIBUTES 
        in the document having the attribute @ism:nonICmarkings containing [ND] 
        then the ISM_RESOURCE_ELEMENT must have @ism:nonICmarkings containing [XD].
        
        Human Readable: USA documents having XD Data and not having ND must have XD at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This rule uses an abstract pattern to consolidate logic. For details on the code
        description, see the abstract pattern.
    </sch:p>
    <sch:param name="attrLocalName" value="nonICmarkings"/>
    <sch:param name="exceptAttrLocalName" value="nonICmarkings"/>
    <sch:param name="value" value="XD"/>
    <sch:param name="exceptValueList" value="('ND')"/>
    <sch:param name="errorMessage" value="'[ISM-ID-00085][Error] USA documents having XD Data and not having ND must have XD at the resource level.'"/>
</sch:pattern>