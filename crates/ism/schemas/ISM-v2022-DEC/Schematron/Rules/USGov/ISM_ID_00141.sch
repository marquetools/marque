<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00141">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00141][Error] If ISM_NSI_EO_APPLIES and:
        1. ISM_RESOURCE_ELEMENT attribute @ism:declassException does not have a value of [25X1-EO-12951], 
        [50X1-HUM], [50X2-WMD], [AEA], [NATO], or [NATO-AEA]
          AND 
        2. ISM_RESOURCE_ELEMENT attribute @ism:declassDate is not specified 
          AND 
        3. ISM_RESOURCE_ELEMENT attribute @ism:declassEvent is not specified 
        
        Human Readable: Documents under E.O. 13526 require declassDate or declassEvent unless 25X1-EO-12951, 
        50X1-HUM, 50X2-WMD, AEA, NATO, or NATO-AEA is specified. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If ISM_NSI_EO_APPLIES, the current element is the ISM_RESOURCE_ELEMENT,
        and attribtue @ism:declassExeption is not specified with a value containing the token
        [25X1-EO-12951], [50X1-HUM], or [50X2-WMD], [AEA], [NATO], or [NATO-AEA] then this rule
        ensures that attribute @ism:declassDate is specified or attribute @ism:declassEvent is
        specified.
    </sch:p>
    <sch:rule id="ISM-ID-00141-R1" context="*[$ISM_NSI_EO_APPLIES and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and not(util:containsAnyOfTheTokens(@ism:declassException, ('25X1-EO-12951', '50X1-HUM', '50X2-WMD', 'AEA', 'NATO', 'NATO-AEA')))]">
        <sch:assert test="@ism:declassDate or @ism:declassEvent" flag="error" role="error">
            [ISM-ID-00141][Error] If ISM_NSI_EO_APPLIES and:
            1. ISM_RESOURCE_ELEMENT attribute @ism:declassException does not have a value of [25X1-EO-12951], 
            [50X1-HUM], [50X2-WMD], [AEA], [NATO], or [NATO-AEA]
            AND 
            2. ISM_RESOURCE_ELEMENT attribute @ism:declassDate is not specified 
            AND 
            3. ISM_RESOURCE_ELEMENT attribute @ism:declassEvent is not specified 
            
            Human Readable: Documents under E.O. 13526 require declassDate or declassEvent unless 25X1-EO-12951, 
            50X1-HUM, 50X2-WMD, AEA, NATO, or NATO-AEA is specified. 
        </sch:assert>
    </sch:rule>
</sch:pattern>