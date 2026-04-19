<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLUP STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00064">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
        ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
        the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
        
        Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
        the resource level. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If IC Markings System Register and Manual marking rules do not apply to the document then this
        rule does not apply and this rule returns true. If the current element has attribute @ism:FGIsourceOpen
        specified and does not have attribute @ism:excludeFromRollup set to true, this rule ensures that
        the resourceElement has one of the following attributes specified: @ism:FGIsourceOpen or @ism:FGIsourceProtected.
    </sch:p>
    <sch:rule id="ISM-ID-00064-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)]">
        <sch:assert test="if(not($ISM_USGOV_RESOURCE)) then true() else if(not(empty($partFGIsourceOpen))) then ($bannerFGIsourceOpen or $bannerFGIsourceProtected) else true()" flag="error" role="error">
            [ISM-ID-00064][Error] If ISM_USGOV_RESOURCE and any element meeting
            ISM_CONTRIBUTES in the document have the attribute @ism:FGIsourceOpen containing any value then
            the ISM_RESOURCE_ELEMENT must have either @ism:FGIsourceOpen or @ism:FGIsourceProtected with a value.
            
            Human Readable: USA documents having FGI Open data must have FGI Open or FGI Protected at
            the resource level. 
        </sch:assert>
    </sch:rule>
</sch:pattern>