<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00228">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00228][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
        [FRD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:atomicEnergyMarking attribute containing [FRD].
        
        Human Readable: USA documents marked FRD at the resource level must have FRD data.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:atomicEnergyMarkings is specified
      with a value containing the value [FRD], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:atomicEnergyMarkings
      with a value containing [FRD].
    </sch:p>
    <sch:rule id="ISM-ID-00228-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))]">
        <sch:assert test="index-of($partAtomicEnergyMarkings_tok,'FRD')&gt;0" flag="error" role="error">
            [ISM-ID-00228][Error] If ISM_USGOV_RESOURCE and attribute @ism:atomicEnergyMarkings of ISM_RESOURCE_ELEMENT contains 
            [FRD] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:atomicEnergyMarking attribute containing [FRD].
            
            Human Readable: USA documents marked FRD at the resource level must have FRD data.
        </sch:assert>
    </sch:rule>
</sch:pattern>