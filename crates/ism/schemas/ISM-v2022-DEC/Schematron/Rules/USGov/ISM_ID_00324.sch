<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00324">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00324][Error] If a document is ISM_USGOV_RESOURCE, it must contain portion markings. 
        
        Human Readable: All valid ISM_USGOV_RESOURCE documents must also contain portion markings. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Make sure that all ISM_USGOV_RESOURCE documents contain at least
        one portion mark if they are not uncaveated UNCLASSIFIED. 
        Allow compilation reason to suffice as an exemption from this rule.
    </sch:p>
    <sch:rule id="ISM-ID-00324-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(@ism:classification='U' and util:isUncaveatedAndNoFDR(.)) and not(@ism:compilationReason)]">
        <sch:assert test="count($partTags) &gt; 0" flag="error" role="error">
            [ISM-ID-00324][Error] If a document is ISM_USGOV_RESOURCE, it must contain portion markings. 
            
            Human Readable: All valid ISM_USGOV_RESOURCE documents must also contain portion markings.
        </sch:assert>
    </sch:rule>  
    
</sch:pattern>