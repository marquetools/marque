<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron"  id="ISM-ID-00511">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00511][Error] arh:Security/@ism:resourceElement attribute must be true.
        
        Human Readable: arh:Security element must contain @ism:resourceElement attribute and @ism:resourceElement
        must equal 'true'.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        Find each instance of arh:Security in the document, test that it has @ism:resourceElement.
    </sch:p>
    <sch:rule id="ISM-ID-00511-R1" context="//arh:Security">
        <sch:assert test="@ism:resourceElement = true()" flag="error" role="error">
            [ISM-ID-00511][Error] arh:Security/@ism:resourceElement attribute must be true.
            
            Human Readable: arh:Security element must contain @ism:resourceElement attribute and @ism:resourceElement
            must equal 'true'.
        </sch:assert>
    </sch:rule>
</sch:pattern>